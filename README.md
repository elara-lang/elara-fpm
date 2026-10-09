# Elara HTTP

HTTP SAPI host that embeds the Elara VM through `elara.h` / `libelara`, running `.ela` scripts behind CGI/FastCGI or a PHP-like cleartext development server (`-S`).

## Highlights

- Keep HTTP and sockets in this host only — never inside the Elara language core
- Discover Elara from a sibling checkout or an install prefix via `ELARA_ROOT` / `ELARA_PREFIX`
- Built-in `-S` / `-t` development server for local cleartext HTTP
- CGI mode (no `-S`) and a FastCGI worker for deploy behind a front-end
- Script API: `req` / `res`, forms, cookies, sessions, redirects, HTML escape
- File-backed sessions under `$TMPDIR/elara/.sessions` (typically `/tmp/elara/.sessions`)

## Prerequisites

- **Rust 1.85+** (`cargo`, `rustc`, `rustfmt`, `clippy`) — [rustup](https://rustup.rs/)
- **Elara** built or installed so `elara.h` and `libelara` are available (see below)

## Installation

### Build from Source

```bash
git clone https://github.com/carlosrabelo/elara-http.git
cd elara-http
make build
```

Install to `~/.local/bin` (default), or system-wide to `/usr/local/bin` (sudo only for the copy):

```bash
make install
make install-system
make uninstall
make uninstall-system
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

## Quick Start

```bash
make discover
make build
make run
```

Open [http://127.0.0.1:8080/hello.ela](http://127.0.0.1:8080/hello.ela). More demos: [`demos/README.md`](demos/README.md). Guides: [`docs/`](docs/). Portuguese: [`README-PT.md`](README-PT.md).

## Usage

### Local development server

```bash
make run
# override listen address / docroot
make run ARGS='-S 127.0.0.1:9090 -t demos'
# or call the binary directly
./bin/elara-http -S 127.0.0.1:8080 -t demos
```

### CGI

With no `-S`, the binary speaks CGI (environment + stdin → stdout):

```bash
./bin/elara-http -t demos
```

Sample front-end config: [`demos/nginx-cgi.conf`](demos/nginx-cgi.conf).

### FastCGI

Use the library FastCGI worker behind a front-end. Sample config: [`demos/nginx-fastcgi.conf`](demos/nginx-fastcgi.conf).

| Mode | How |
| --- | --- |
| Local cleartext | `elara-http -S 127.0.0.1:8080 -t <docroot>` |
| CGI | `elara-http -t <docroot>` with CGI env + stdin (no `-S`) |
| FastCGI | library worker (`run_fastcgi_worker`); see `demos/nginx-fastcgi.conf` |

## Project Layout

```
Makefile       # Root orchestrator
.make/         # Build, test, quality, Elara discovery
elara_http/    # Host Cargo package + elara-http binary
bin/           # Release binary (git-ignored except .gitkeep)
demos/         # Sample .ela scripts and nginx snippets
docs/          # Public guides (EN + PT)
```

## Development

```bash
make build             # Release-build and copy binary to bin/elara-http
make test              # Run cargo test
make quality           # Format, clippy, and test
make install           # Install binary to ~/.local/bin
make install-system    # Install binary to /usr/local/bin
make uninstall         # Remove from ~/.local/bin
make uninstall-system  # Remove from /usr/local/bin
```

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.
