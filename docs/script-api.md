# Script API overview

Scripts receive `req` and `res` (and optionally `session`) for each request.

## `req`

| Field | Meaning |
| --- | --- |
| `method` | Uppercased HTTP method |
| `path` | Request path |
| `query` | Raw query string (no `?`) |
| `args` | Parsed query map (percent-decoded; `+` is space; **last key wins**) |
| `headers` | Lowercased header map |
| `body` | Raw body string |
| `host` | Request host (`Host` / `HTTP_HOST` / `SERVER_NAME`); may include `:port`; empty when absent |
| `scheme` | `http` or `https` (`-S` is always `http`) |
| `remote` | Client address without port (peer / `REMOTE_ADDR`); empty when absent |
| `form` | Urlencoded / multipart text fields (body only; not merged with `args`) |
| `cookies` | Parsed `Cookie` header |
| `json` | Parsed JSON object (or `nil`) |
| `files` | Multipart uploads (`name`, `filename`, `content_type`, `path`, `size`) |

The host does **not** trust `X-Forwarded-Proto` or `X-Forwarded-For` for `scheme` / `remote`. Put TLS and trusted client identity on the front-end proxy.

## `res`

| Call | Effect |
| --- | --- |
| `res.set_status(code)` | HTTP status |
| `res.header(name, value)` | Response header |
| `res.write(chunk)` | Append body text |
| `res.cookie(name, value)` | `Set-Cookie` with defaults |
| `res.cookie_ex(...)` | Cookie with path / flags / max-age |
| `res.redirect(url)` | 302 + `Location` (optional status 301/303/307/308) |
| `res.render(path, ctx)` | Append a rendered HTML template (see below) |

Writes after `redirect` are ignored. `res.write` stays raw; only template interpolation auto-escapes.

## Templates (`res.render`)

```ela
res.render("hello.htm", { name: "ada" })
```

- `path` is relative to the template directory (default `<docroot>/templates`). Override with `--template-dir`. Scripts cannot choose the directory.
- `ctx` is a table (omitted/`nil` means `{}`). Render appends to the body; it does not set status or `Content-Type`.
- Files under the template directory are not served as URLs (`404`).

| Form | Effect |
| --- | --- |
| `{{ name }}` | Insert value, HTML-escaped (`& < > " '`) |
| `{{ name \| safe }}` | Insert value with no escape |
| `{{ a.b }}` | Nested table lookup; missing keys insert empty string |
| `{% if cond %} … {% else %} … {% endif %}` | `else` optional; falsey: `nil`, `false`, `""`, empty table |
| `{% for item in items %} … {% endfor %}` | Walk integer keys in ascending order |
| `{% include "part.htm" %}` | Nested render (same table); depth capped at 8 |
| `{# … #}` | Comment (omitted from output) |

Insertable values: string, int, bool, nil. Inserting a table is a host `500`. Path escape or missing file is also `500` (no absolute path in the body).

## Helpers

- `html_escape(s)` — escape `& < > " '` for HTML text/attributes
- `session.get(key)` / `session.set(key, value)` / `session.destroy()` — file-backed session (when enabled by the host). Stored values may be string, int, bool, nil, or tables of those kinds (nested tables up to depth 8). Floats, functions, userdata, and deeper tables fail `session.set` without changing the previous session file.

## Example

```ela
res.header("Content-Type", "text/plain; charset=utf-8")
res.write("hello ")
res.write(req.method)
```

See also [`demos/`](../demos/).
