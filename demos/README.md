# Elara HTTP demos

Run with the built-in cleartext server from the repo root (after `make build`):

```bash
cargo run --manifest-path elara_http/Cargo.toml --bin elara-http -- -S 127.0.0.1:8080 -t demos
```

Then open `http://127.0.0.1:8080/hello.ela`.

| Script | Purpose |
| --- | --- |
| `hello.ela` | Write a plain hello body |
| `echo.ela` | Echo method, path, query, and raw body |
| `form.ela` | Read `name` from a urlencoded POST |
| `cookie.ela` | Set then read a demo cookie |
| `session.ela` | Increment a session counter across requests |
| `redirect.ela` | Redirect to `hello.ela` |
| `style.css` | Static CSS next to scripts |
| `index.ela` | Docroot index that links demos |

Front-end deploy examples: see `nginx-cgi.conf` and `nginx-fastcgi.conf`.
