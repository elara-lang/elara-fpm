# Local development server (`-S`)

```bash
make build
./bin/elara-fpm -S 127.0.0.1:8080 -t demos
```

| Flag | Meaning |
| --- | --- |
| `-S <addr>:<port>` | Listen for HTTP/1.1 cleartext |
| `-t <dir>` | Document root (default: current directory) |

On start, the process prints the listen URL and document root to **stderr**.

## Request framing

- With `Content-Length`, `-S` reads the full body (across multiple socket reads) up to the host body limit (default 1 MiB). Over that limit returns HTTP `413` and closes the connection.
- Without `Content-Length`, the body is empty (including `POST`).
- `Transfer-Encoding: chunked` is not implemented; the server responds `501` and closes.
- HTTP/1.1 keep-alive: if the client omits `Connection: close`, the connection stays open for the next request. Leftover bytes after one request start the next. HTTP/1.0 or `Connection: close` ends the connection after the response.

## HEAD

`HEAD` is handled like `GET`: the same route runs (script or static), and the response keeps the same status and headers. The host sets `Content-Length` to the byte length of the body that `GET` would have sent, then sends an empty body. Scripts still execute, so session and other side effects match `GET`.

## Limits

- Cleartext only — no TLS, no HTTP/2
- Intended for local development, not public production
- Each request gets a fresh sandboxed VM state
- `.ela` source and `.elac` bytecode scripts run; other files under the docroot are served as static content
- Directory URLs pick the first existing index: `index.ela` (run), then `index.elac` (run), then `index.html` (static). Otherwise `404`. No directory listings.
- Paths cannot escape the document root (`..` is rejected)

## Demos

See [`demos/README.md`](../demos/README.md).
