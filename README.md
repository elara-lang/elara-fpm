# Elara HTTP

HTTP **SAPI host** that embeds the Elara VM through `elara.h` / `libelara`. It runs `.ela` scripts behind a front-end (CGI / FastCGI) or locally with a PHP-like cleartext development server (`-S`). This host is **not** a production TLS/HTTP/2 server — terminate TLS on nginx, Caddy, or Apache.

## Highlights

- Keep HTTP and sockets in this host only — never inside the Elara language core
- Discover Elara from a sibling checkout or an install prefix via `ELARA_ROOT` / `ELARA_PREFIX`
- Built-in `-S` / `-t` development server for local cleartext HTTP
- CGI mode (no `-S`) and a FastCGI worker for deploy behind a front-end
- Script API: `req` / `res`, forms, cookies, sessions, redirects, HTML escape

## Prerequisites

- **Rust 1.85+** (`cargo`, `rustc`, `rustfmt`, `clippy`) — [rustup](https://rustup.rs/)
- **Elara** built or installed so `elara.h` and `libelara` are available (see below)

## Installation

### Build this repository

```bash
git clone https://github.com/carlosrabelo/elara-http.git
cd elara-http
make build
make test
```

### Elara dependency

Elara HTTP does not vendor the language tree. The build resolves headers and libraries in this order:

| Mode | When | Headers | Library |
| --- | --- | --- | --- |
| Install | `ELARA_PREFIX` is set | `$ELARA_PREFIX/include/elara.h` | `$ELARA_PREFIX/lib/libelara.*` |
| Sibling | `ELARA_ROOT` or `../elara` | `$ELARA_ROOT/elara/include/elara.h` | `$ELARA_ROOT/bin` or `elara/target/release` |

If both `ELARA_PREFIX` and `ELARA_ROOT` are set, **install mode wins**.

```bash
# Sibling (default when ../elara exists)
make discover

# Explicit sibling root
ELARA_ROOT=/path/to/elara make discover

# Prefix install
ELARA_PREFIX=/usr/local make discover
```

Missing Elara fails with a non-zero exit and names `ELARA_ROOT` / `ELARA_PREFIX`.

Build Elara first if needed:

```bash
cd ../elara && make build
```

## Quick Start (`-S`)

```bash
make discover
make build
cargo run --manifest-path elara_http/Cargo.toml --bin elara-http -- -S 127.0.0.1:8080 -t demos
```

Open [http://127.0.0.1:8080/hello.ela](http://127.0.0.1:8080/hello.ela). More demos: [`demos/README.md`](demos/README.md).

Deeper guides: [`docs/`](docs/) (script API, local server, deployment). Portuguese: [`README-PT.md`](README-PT.md).

## Usage

| Mode | How |
| --- | --- |
| Local cleartext | `elara-http -S 127.0.0.1:8080 -t <docroot>` |
| CGI | `elara-http -t <docroot>` with CGI env + stdin (no `-S`) |
| FastCGI | library worker (`run_fastcgi_worker`); see `demos/nginx-fastcgi.conf` |

## Development

```bash
make help       # targets + Elara env docs
make discover   # print resolved Elara paths
make check      # cargo check
make test       # cargo test (needs libelara via make)
make lint       # clippy -D warnings
make fmt        # rustfmt
make quality    # fmt check + clippy + test
make clean
```

## Project Layout

| Path | Role |
| --- | --- |
| `Makefile` | Root orchestrator |
| `.make/` | Build, test, quality, Elara discovery |
| `elara_http/` | Host Cargo package + `elara-http` binary |
| `demos/` | Sample `.ela` scripts and front-end snippets |
| `docs/` | Public guides (EN + PT) |

## License

MIT
