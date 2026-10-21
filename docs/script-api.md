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
| `form` | Urlencoded / multipart text fields (body only; not merged with `args`) |
| `cookies` | Parsed `Cookie` header |
| `json` | Parsed JSON object (or `nil`) |
| `files` | Multipart uploads (`name`, `filename`, `content_type`, `path`, `size`) |

## `res`

| Call | Effect |
| --- | --- |
| `res.set_status(code)` | HTTP status |
| `res.header(name, value)` | Response header |
| `res.write(chunk)` | Append body text |
| `res.cookie(name, value)` | `Set-Cookie` with defaults |
| `res.cookie_ex(...)` | Cookie with path / flags / max-age |
| `res.redirect(url)` | 302 + `Location` (optional status 301/303/307/308) |

Writes after `redirect` are ignored.

## Helpers

- `html_escape(s)` — escape `& < > " '` for HTML text/attributes
- `session.get(key)` / `session.set(key, value)` / `session.destroy()` — file-backed session (when enabled by the host)

## Example

```ela
res.header("Content-Type", "text/plain; charset=utf-8")
res.write("hello ")
res.write(req.method)
```

See also [`demos/`](../demos/).
