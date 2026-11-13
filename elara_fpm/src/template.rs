//! HTML page templates for `res.render` (not Elara).

#![allow(clippy::missing_safety_doc)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::ffi::{
    elara_CallContext, elara_State, elara_Status, elara_arg, elara_argc, elara_return,
    elara_val_nil, value_as_string,
};
use crate::helpers::html_escape;
use crate::response::response_from_ctx;

const MAX_INCLUDE_DEPTH: u32 = 8;

/// Values that templates can look up and insert.
#[derive(Debug, Clone, PartialEq)]
pub enum TmplValue {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<TmplValue>),
    Object(BTreeMap<String, TmplValue>),
}

impl TmplValue {
    fn is_falsey(&self) -> bool {
        match self {
            TmplValue::Null | TmplValue::Bool(false) => true,
            TmplValue::Str(s) if s.is_empty() => true,
            TmplValue::Object(m) if m.is_empty() => true,
            _ => false,
        }
    }

    fn as_insert_text(&self) -> Result<String, String> {
        match self {
            TmplValue::Null => Ok(String::new()),
            TmplValue::Bool(b) => Ok(if *b { "true" } else { "false" }.into()),
            TmplValue::Int(n) => Ok(n.to_string()),
            TmplValue::Str(s) => Ok(s.clone()),
            TmplValue::Array(_) | TmplValue::Object(_) => {
                Err("cannot insert a table into a template".into())
            }
        }
    }

    fn get_path<'a>(&'a self, path: &[&str]) -> &'a TmplValue {
        let mut cur = self;
        for key in path {
            match cur {
                TmplValue::Object(m) => {
                    cur = m.get(*key).unwrap_or(&TmplValue::Null);
                }
                _ => return &TmplValue::Null,
            }
        }
        cur
    }
}

#[derive(Debug, Clone)]
enum Node {
    Text(String),
    Interp {
        path: Vec<String>,
        safe: bool,
    },
    If {
        cond: Vec<String>,
        then_body: Vec<Node>,
        else_body: Vec<Node>,
    },
    For {
        item: String,
        list: Vec<String>,
        body: Vec<Node>,
    },
    Include(String),
}

#[derive(Debug, Clone)]
struct CompiledTemplate {
    nodes: Vec<Node>,
    mtime: SystemTime,
}

struct TemplateCache {
    entries: BTreeMap<PathBuf, CompiledTemplate>,
}

impl TemplateCache {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    fn get_or_compile(&mut self, path: &Path) -> Result<CompiledTemplate, String> {
        let meta = fs::metadata(path).map_err(|_| "template not found".to_string())?;
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if let Some(c) = self.entries.get(path)
            && c.mtime == mtime
        {
            return Ok(c.clone());
        }
        let src = fs::read_to_string(path).map_err(|_| "template not found".to_string())?;
        let nodes = parse_template(&src)?;
        let compiled = CompiledTemplate { nodes, mtime };
        self.entries.insert(path.to_path_buf(), compiled.clone());
        Ok(compiled)
    }
}

static TEMPLATE_CACHE: Mutex<Option<TemplateCache>> = Mutex::new(None);

fn with_cache<R>(f: impl FnOnce(&mut TemplateCache) -> R) -> R {
    let mut guard = TEMPLATE_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(TemplateCache::new());
    }
    f(guard.as_mut().unwrap())
}

/// Clear the process template cache (tests).
pub fn clear_template_cache() {
    with_cache(|c| c.entries.clear());
}

thread_local! {
    static TEMPLATE_DIR: std::cell::RefCell<PathBuf> =
        const { std::cell::RefCell::new(PathBuf::new()) };
}

/// Set the template directory for the current request thread.
pub fn set_template_dir(dir: PathBuf) {
    TEMPLATE_DIR.with(|d| *d.borrow_mut() = dir);
}

/// Default template directory: `<docroot>/templates`.
pub fn default_template_dir(docroot: &Path) -> PathBuf {
    docroot.join("templates")
}

/// True when `path` resolves inside the template directory.
pub fn path_is_under_template_dir(path: &Path, template_dir: &Path) -> bool {
    let Ok(root) = fs::canonicalize(template_dir) else {
        return false;
    };
    let Ok(canon) = fs::canonicalize(path) else {
        return false;
    };
    canon.starts_with(&root)
}

/// Resolve a relative template path under `root` (no `..`, no absolute).
pub fn resolve_template_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty() || Path::new(rel).is_absolute() {
        return Err("invalid template path".into());
    }
    let root = fs::canonicalize(root).map_err(|_| "template directory missing".to_string())?;
    let mut out = PathBuf::new();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(s) => {
                let name = s.to_string_lossy();
                if name.starts_with('.') {
                    return Err("invalid template path".into());
                }
                out.push(s);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("invalid template path".into());
            }
        }
    }
    let joined = root.join(&out);
    let resolved = if joined.exists() {
        fs::canonicalize(&joined).map_err(|_| "template not found".to_string())?
    } else {
        return Err("template not found".into());
    };
    if !resolved.starts_with(&root) {
        return Err("invalid template path".into());
    }
    Ok(resolved)
}

fn parse_template(src: &str) -> Result<Vec<Node>, String> {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    parse_seq(bytes, &mut i, &[])
}

fn parse_seq(bytes: &[u8], i: &mut usize, stop: &[&str]) -> Result<Vec<Node>, String> {
    let mut nodes = Vec::new();
    while *i < bytes.len() {
        if let Some(tag) = peek_tag(bytes, *i)
            && stop.iter().any(|s| *s == tag)
        {
            return Ok(nodes);
        }
        nodes.push(parse_item(bytes, i)?);
    }
    if !stop.is_empty() {
        return Err("unclosed template block".into());
    }
    Ok(nodes)
}

fn parse_item(bytes: &[u8], i: &mut usize) -> Result<Node, String> {
    if *i >= bytes.len() {
        return Err("unexpected end".into());
    }
    if bytes[*i] == b'{' && *i + 1 < bytes.len() {
        match bytes[*i + 1] {
            b'{' => {
                *i += 2;
                let (path, safe) = parse_interp(bytes, i)?;
                return Ok(Node::Interp { path, safe });
            }
            b'#' => {
                *i += 2;
                skip_until(bytes, i, b"#}")?;
                // Comment: skip by reading the next item (or empty text at EOF).
                if *i >= bytes.len() {
                    return Ok(Node::Text(String::new()));
                }
                return parse_item(bytes, i);
            }
            b'%' => {
                *i += 2;
                let tag = read_tag_body(bytes, i)?;
                let trimmed = tag.trim();
                if let Some(rest) = trimmed.strip_prefix("if ") {
                    let cond = parse_path(rest.trim())?;
                    let then_body = parse_seq(bytes, i, &["else", "endif"])?;
                    let else_body = if peek_tag(bytes, *i).as_deref() == Some("else") {
                        let _ = consume_tag(bytes, i)?;
                        parse_seq(bytes, i, &["endif"])?
                    } else {
                        Vec::new()
                    };
                    expect_tag(bytes, i, "endif")?;
                    return Ok(Node::If {
                        cond,
                        then_body,
                        else_body,
                    });
                }
                if let Some(rest) = trimmed.strip_prefix("for ") {
                    let (item, list) = parse_for_header(rest.trim())?;
                    let body = parse_seq(bytes, i, &["endfor"])?;
                    expect_tag(bytes, i, "endfor")?;
                    return Ok(Node::For { item, list, body });
                }
                if let Some(rest) = trimmed.strip_prefix("include ") {
                    return Ok(Node::Include(parse_include_path(rest.trim())?));
                }
                return Err(format!("unknown template tag: {trimmed}"));
            }
            _ => {}
        }
    }
    let start = *i;
    *i += 1;
    while *i < bytes.len() {
        if bytes[*i] == b'{' && *i + 1 < bytes.len() {
            let n = bytes[*i + 1];
            if n == b'{' || n == b'%' || n == b'#' {
                break;
            }
        }
        *i += 1;
    }
    Ok(Node::Text(
        String::from_utf8_lossy(&bytes[start..*i]).into_owned(),
    ))
}

fn peek_tag(bytes: &[u8], i: usize) -> Option<String> {
    if i + 1 >= bytes.len() || bytes[i] != b'{' || bytes[i + 1] != b'%' {
        return None;
    }
    let mut j = i + 2;
    let body = read_tag_body(bytes, &mut j).ok()?;
    Some(body.trim().to_string())
}

fn consume_tag(bytes: &[u8], i: &mut usize) -> Result<String, String> {
    if *i + 1 >= bytes.len() || bytes[*i] != b'{' || bytes[*i + 1] != b'%' {
        return Err("expected tag".into());
    }
    *i += 2;
    Ok(read_tag_body(bytes, i)?.trim().to_string())
}

fn expect_tag(bytes: &[u8], i: &mut usize, name: &str) -> Result<(), String> {
    let got = consume_tag(bytes, i)?;
    if got != name {
        return Err(format!("expected {{% {name} %}}, got {{% {got} %}}"));
    }
    Ok(())
}

fn read_tag_body(bytes: &[u8], i: &mut usize) -> Result<String, String> {
    let start = *i;
    while *i + 1 < bytes.len() {
        if bytes[*i] == b'%' && bytes[*i + 1] == b'}' {
            let body = String::from_utf8_lossy(&bytes[start..*i]).into_owned();
            *i += 2;
            return Ok(body);
        }
        *i += 1;
    }
    Err("unclosed {% tag".into())
}

fn skip_until(bytes: &[u8], i: &mut usize, end: &[u8]) -> Result<(), String> {
    while *i + end.len() <= bytes.len() {
        if &bytes[*i..*i + end.len()] == end {
            *i += end.len();
            return Ok(());
        }
        *i += 1;
    }
    Err("unclosed template comment".into())
}

fn parse_interp(bytes: &[u8], i: &mut usize) -> Result<(Vec<String>, bool), String> {
    let start = *i;
    while *i + 1 < bytes.len() {
        if bytes[*i] == b'}' && bytes[*i + 1] == b'}' {
            let body = String::from_utf8_lossy(&bytes[start..*i])
                .trim()
                .to_string();
            *i += 2;
            return parse_interp_body(&body);
        }
        *i += 1;
    }
    Err("unclosed {{".into())
}

fn parse_interp_body(body: &str) -> Result<(Vec<String>, bool), String> {
    let (expr, safe) = if let Some((left, right)) = body.split_once('|') {
        let filter = right.trim();
        if filter != "safe" {
            return Err(format!("unknown filter: {filter}"));
        }
        (left.trim(), true)
    } else {
        (body.trim(), false)
    };
    Ok((parse_path(expr)?, safe))
}

fn parse_path(expr: &str) -> Result<Vec<String>, String> {
    if expr.is_empty() {
        return Err("empty template path".into());
    }
    let parts: Vec<String> = expr
        .split('.')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("empty template path".into());
    }
    for p in &parts {
        if !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("invalid path segment: {p}"));
        }
    }
    Ok(parts)
}

fn parse_for_header(rest: &str) -> Result<(String, Vec<String>), String> {
    let mut parts = rest.split_whitespace();
    let item = parts.next().ok_or("bad for tag")?.to_string();
    let inn = parts.next().ok_or("bad for tag")?;
    if inn != "in" {
        return Err("expected 'in' in for tag".into());
    }
    let list = parts.next().ok_or("bad for tag")?;
    if parts.next().is_some() {
        return Err("bad for tag".into());
    }
    if !item.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("bad for loop variable".into());
    }
    Ok((item, parse_path(list)?))
}

fn parse_include_path(rest: &str) -> Result<String, String> {
    let rest = rest.trim();
    if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
        return Ok(rest[1..rest.len() - 1].to_string());
    }
    Err("include path must be a quoted string".into())
}

fn render_nodes(
    nodes: &[Node],
    root: &TmplValue,
    locals: &BTreeMap<String, TmplValue>,
    template_root: &Path,
    depth: u32,
) -> Result<String, String> {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Text(t) => out.push_str(t),
            Node::Interp { path, safe } => {
                let refs: Vec<&str> = path.iter().map(String::as_str).collect();
                let val = lookup(root, locals, &refs);
                let text = val.as_insert_text()?;
                if *safe {
                    out.push_str(&text);
                } else {
                    out.push_str(&html_escape(&text));
                }
            }
            Node::If {
                cond,
                then_body,
                else_body,
            } => {
                let refs: Vec<&str> = cond.iter().map(String::as_str).collect();
                let val = lookup(root, locals, &refs);
                let branch = if !val.is_falsey() {
                    then_body
                } else {
                    else_body
                };
                out.push_str(&render_nodes(branch, root, locals, template_root, depth)?);
            }
            Node::For { item, list, body } => {
                let refs: Vec<&str> = list.iter().map(String::as_str).collect();
                let val = lookup(root, locals, &refs);
                let items = match val {
                    TmplValue::Array(a) => a.clone(),
                    TmplValue::Object(m) => {
                        let mut keys: Vec<i64> = Vec::new();
                        for k in m.keys() {
                            if let Ok(n) = k.parse::<i64>() {
                                keys.push(n);
                            }
                        }
                        keys.sort();
                        keys.into_iter()
                            .filter_map(|k| m.get(&k.to_string()).cloned())
                            .collect()
                    }
                    _ => Vec::new(),
                };
                for it in items {
                    let mut nested = locals.clone();
                    nested.insert(item.clone(), it);
                    out.push_str(&render_nodes(body, root, &nested, template_root, depth)?);
                }
            }
            Node::Include(rel) => {
                if depth >= MAX_INCLUDE_DEPTH {
                    return Err("include depth exceeded".into());
                }
                let path = resolve_template_path(template_root, rel)?;
                let compiled = with_cache(|cache| cache.get_or_compile(&path))?;
                out.push_str(&render_nodes(
                    &compiled.nodes,
                    root,
                    locals,
                    template_root,
                    depth + 1,
                )?);
            }
        }
    }
    Ok(out)
}

fn lookup<'a>(
    root: &'a TmplValue,
    locals: &'a BTreeMap<String, TmplValue>,
    path: &[&str],
) -> &'a TmplValue {
    if path.is_empty() {
        return &TmplValue::Null;
    }
    if let Some(v) = locals.get(path[0]) {
        if path.len() == 1 {
            return v;
        }
        return v.get_path(&path[1..]);
    }
    root.get_path(path)
}

/// Parse a JSON value (object/array/scalar) into [`TmplValue`].
pub fn parse_tmpl_json(input: &str) -> Result<TmplValue, String> {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let v = parse_json_val(bytes, &mut i)?;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i != bytes.len() {
        return Err("trailing junk in JSON".into());
    }
    Ok(v)
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn parse_json_val(bytes: &[u8], i: &mut usize) -> Result<TmplValue, String> {
    skip_ws(bytes, i);
    if *i >= bytes.len() {
        return Err("unexpected end of JSON".into());
    }
    match bytes[*i] {
        b'n' => {
            if bytes.get(*i..*i + 4) == Some(b"null") {
                *i += 4;
                Ok(TmplValue::Null)
            } else {
                Err("bad null".into())
            }
        }
        b't' => {
            if bytes.get(*i..*i + 4) == Some(b"true") {
                *i += 4;
                Ok(TmplValue::Bool(true))
            } else {
                Err("bad true".into())
            }
        }
        b'f' => {
            if bytes.get(*i..*i + 5) == Some(b"false") {
                *i += 5;
                Ok(TmplValue::Bool(false))
            } else {
                Err("bad false".into())
            }
        }
        b'"' => Ok(TmplValue::Str(parse_json_string(bytes, i)?)),
        b'{' => parse_json_object(bytes, i),
        b'[' => parse_json_array(bytes, i),
        b'-' | b'0'..=b'9' => {
            let start = *i;
            if bytes[*i] == b'-' {
                *i += 1;
            }
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
            let s = std::str::from_utf8(&bytes[start..*i]).map_err(|_| "bad number")?;
            let n: i64 = s.parse().map_err(|_| "bad number")?;
            Ok(TmplValue::Int(n))
        }
        _ => Err("unexpected JSON value".into()),
    }
}

fn parse_json_string(bytes: &[u8], i: &mut usize) -> Result<String, String> {
    if *i >= bytes.len() || bytes[*i] != b'"' {
        return Err("expected string".into());
    }
    *i += 1;
    let mut out = String::new();
    while *i < bytes.len() {
        match bytes[*i] {
            b'"' => {
                *i += 1;
                return Ok(out);
            }
            b'\\' => {
                *i += 1;
                if *i >= bytes.len() {
                    return Err("truncated escape".into());
                }
                match bytes[*i] {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    _ => return Err("bad escape".into()),
                }
                *i += 1;
            }
            b => {
                out.push(b as char);
                *i += 1;
            }
        }
    }
    Err("unterminated string".into())
}

fn parse_json_object(bytes: &[u8], i: &mut usize) -> Result<TmplValue, String> {
    *i += 1;
    let mut map = BTreeMap::new();
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(TmplValue::Object(map));
        }
        let key = parse_json_string(bytes, i)?;
        skip_ws(bytes, i);
        if *i >= bytes.len() || bytes[*i] != b':' {
            return Err("expected :".into());
        }
        *i += 1;
        let val = parse_json_val(bytes, i)?;
        map.insert(key, val);
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(TmplValue::Object(map));
        }
        return Err("expected , or }".into());
    }
}

fn parse_json_array(bytes: &[u8], i: &mut usize) -> Result<TmplValue, String> {
    *i += 1;
    let mut arr = Vec::new();
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b']' {
            *i += 1;
            return Ok(TmplValue::Array(arr));
        }
        arr.push(parse_json_val(bytes, i)?);
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b']' {
            *i += 1;
            return Ok(TmplValue::Array(arr));
        }
        return Err("expected , or ]".into());
    }
}

/// Load, cache, and render a template file under `template_root`.
pub fn render_file(template_root: &Path, rel: &str, ctx: &TmplValue) -> Result<String, String> {
    let path = resolve_template_path(template_root, rel)?;
    let locals = BTreeMap::new();
    let compiled = with_cache(|cache| cache.get_or_compile(&path))?;
    render_nodes(&compiled.nodes, ctx, &locals, template_root, 0)
}

/// `res.render(path, json_ctx)` — appends rendered bytes; errors surface as runtime failure.
pub unsafe extern "C" fn native_res_render(
    s: *mut elara_State,
    ctx: *mut elara_CallContext,
) -> elara_Status {
    unsafe {
        let buf = match response_from_ctx(ctx) {
            Some(b) => b,
            None => return elara_Status::ErrRuntime,
        };
        if buf.finalized {
            elara_return(ctx, elara_val_nil());
            return elara_Status::Ok;
        }
        let argc = elara_argc(ctx);
        if argc < 1 {
            return elara_Status::ErrRuntime;
        }
        let path = match value_as_string(s, elara_arg(ctx, 0)) {
            Ok(v) => v,
            Err(_) => return elara_Status::ErrRuntime,
        };
        let json = if argc >= 2 {
            match value_as_string(s, elara_arg(ctx, 1)) {
                Ok(v) => v,
                Err(_) => return elara_Status::ErrRuntime,
            }
        } else {
            "{}".into()
        };
        let tmpl_ctx = match parse_tmpl_json(&json) {
            Ok(v @ TmplValue::Object(_)) => v,
            _ => return elara_Status::ErrRuntime,
        };
        let dir = TEMPLATE_DIR.with(|d| d.borrow().clone());
        if dir.as_os_str().is_empty() {
            return elara_Status::ErrRuntime;
        }
        match render_file(&dir, &path, &tmpl_ctx) {
            Ok(html) => {
                buf.body.extend_from_slice(html.as_bytes());
                elara_return(ctx, elara_val_nil());
                elara_Status::Ok
            }
            Err(_) => elara_Status::ErrRuntime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_escaped_and_safe() {
        let nodes = parse_template("<p>Hello {{ name }}</p>").unwrap();
        let ctx = TmplValue::Object(BTreeMap::from([(
            "name".into(),
            TmplValue::Str("<ada>".into()),
        )]));
        let out = render_nodes(&nodes, &ctx, &BTreeMap::new(), Path::new("."), 0).unwrap();
        assert_eq!(out, "<p>Hello &lt;ada&gt;</p>");

        let nodes = parse_template("{{ name | safe }}").unwrap();
        let out = render_nodes(&nodes, &ctx, &BTreeMap::new(), Path::new("."), 0).unwrap();
        assert_eq!(out, "<ada>");
    }

    #[test]
    fn missing_key_is_empty() {
        let nodes = parse_template("x{{ missing }}y").unwrap();
        let ctx = TmplValue::Object(BTreeMap::new());
        let out = render_nodes(&nodes, &ctx, &BTreeMap::new(), Path::new("."), 0).unwrap();
        assert_eq!(out, "xy");
    }
}
