# Elara FPM demos

Run with the built-in cleartext server from the repo root (after `make build`):

```bash
make build
make run
# or: ./bin/elara-fpm -S 127.0.0.1:8080 -t demos
```

Then open `http://127.0.0.1:8080/` (or `/hello.ela`).

| Path | Purpose |
| --- | --- |
| `hello.ela` | Write a plain hello body |
| `echo.ela` | Echo method, path, query, `req.args`, host/scheme/remote, and raw body |
| `form.ela` | Read `name` from a urlencoded POST |
| `cookie.ela` | Set then read a demo cookie |
| `session.ela` | Increment an int session counter (and a bool flag) across requests |
| `redirect.ela` | Redirect to `hello.ela` |
| `template.ela` | `res.render` with escape, `if` / `for`, and an include under `templates/` |
| `templates/` | Template files (not served as URLs — expect `404`) |
| `static-index/` | Directory with only `index.html` (no index script) |
| `hi.elac` | Precompiled bytecode sample (body `elac-ok`); equivalent source is `res.write("elac-ok\n")` wrapped by the host |
| `style.css` | Static CSS next to scripts |
| `index.ela` | Docroot index that links demos |

## Access log

Every finished `-S`, CGI, or FastCGI request prints one stderr line:

```text
<method> <path> <status> <elapsed_ms>
```

Example: `GET /hello.ela 200 3`. The path omits `?` and the query string.

## HEAD

`HEAD` on any demo path matches the `GET` status and headers, with an empty body (scripts still run).

## Request framing (`-S`)

- With `Content-Length`, `-S` reads the full body — try `POST` to `/echo.ela`.
- HTTP/1.1 keep-alive reuses the connection unless the client sends `Connection: close`.
- `Transfer-Encoding: chunked` is not implemented (`501`). Bodies over the limit are `413`.

## FastCGI (`-F`)

Do not combine `-S` and `-F`. Start a worker against this docroot, then point nginx at it (see `nginx-fastcgi.conf`):

```bash
./bin/elara-fpm -F 127.0.0.1:9000 -t demos
# or: ./bin/elara-fpm -F /tmp/elara-fpm.sock -t demos
```

CGI without `-S`/`-F` still works; see `nginx-cgi.conf`.

## Operator flags

These apply to `-S`, CGI, and `-F` for the whole process (see also `docs/deployment.md`):

| Flag | Role |
| --- | --- |
| `--gas <n>` | Per-request gas limit |
| `--mem <bytes>` | Per-request memory limit |
| `--max-body <bytes>` | Maximum request body size |
| `--session-dir <path>` | Session file-store directory |
| `--session-secure` | Set `Secure` on the session cookie |
| `--session-samesite Lax\|Strict\|None` | Session cookie `SameSite` |
| `--allow-sys-env` | Grant `ELARA_CAP_SYS_ENV` |

Example with a tight body limit:

```bash
./bin/elara-fpm -S 127.0.0.1:8080 -t demos --max-body 1024
```

Portuguese: [`README-PT.md`](README-PT.md).
