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

Writes after `redirect` are ignored.

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
