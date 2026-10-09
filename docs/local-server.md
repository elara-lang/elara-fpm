# Local development server (`-S`)

```bash
make build
./bin/elara-http -S 127.0.0.1:8080 -t demos
```

| Flag | Meaning |
| --- | --- |
| `-S <addr>:<port>` | Listen for HTTP/1.1 cleartext |
| `-t <dir>` | Document root (default: current directory) |

On start, the process prints the listen URL and document root to **stderr**.

## Limits

- Cleartext only — no TLS, no HTTP/2
- Intended for local development, not public production
- Each request gets a fresh sandboxed VM state
- `.ela` scripts run; other files under the docroot are served as static content
- Paths cannot escape the document root (`..` is rejected)

## Demos

See [`demos/README.md`](../demos/README.md).
