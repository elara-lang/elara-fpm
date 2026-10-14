# Elara FPM

HTTP SAPI host that embeds the Elara VM through `elara.h` / `libelara`, running `.ela` scripts behind CGI/FastCGI or a PHP-like cleartext development server (`-S`).

## Highlights

- Keep HTTP and sockets in this host only — never inside the Elara language core
- Discover Elara from `/usr/local`, another prefix (`ELARA_PREFIX`), or a sibling checkout (`ELARA_ROOT`)
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
git clone https://github.com/carlosrabelo/elara-fpm.git
cd elara-fpm
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

Elara FPM does not vendor the language tree. The build resolves headers and libraries in this order:

| Mode | When | Headers | Library |
| --- | --- | --- | --- |
| Install | `/usr/local` contains the install, or `ELARA_PREFIX` is set | `$prefix/include/elara.h` | `$prefix/lib/libelara.*` |
| Sibling | `ELARA_ROOT` is set, or `../elara` when no install is found | `$ELARA_ROOT/elara/include/elara.h` | `$ELARA_ROOT/bin` or `elara/target/release` |

With neither variable set, an install under `/usr/local` is used when `include/elara.h` and `lib/libelara.*` are there; otherwise the sibling checkout is used. If both `ELARA_PREFIX` and `ELARA_ROOT` are set, **install mode wins**. `ELARA_ROOT` selects the checkout even when `/usr/local` also has Elara.

```bash
# Default: /usr/local when Elara is installed there
make discover

# Explicit prefix
ELARA_PREFIX=/opt/elara make discover

# Sibling checkout (overrides /usr/local)
ELARA_ROOT=/path/to/elara make discover
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
./bin/elara-fpm -S 127.0.0.1:8080 -t demos
```

### CGI

With no `-S`, the binary speaks CGI (environment + stdin → stdout):

```bash
./bin/elara-fpm -t demos
```

Sample front-end config: [`demos/nginx-cgi.conf`](demos/nginx-cgi.conf).

### FastCGI

Use the library FastCGI worker behind a front-end. Sample config: [`demos/nginx-fastcgi.conf`](demos/nginx-fastcgi.conf).

| Mode | How |
| --- | --- |
| Local cleartext | `elara-fpm -S 127.0.0.1:8080 -t <docroot>` |
| CGI | `elara-fpm -t <docroot>` with CGI env + stdin (no `-S`) |
| FastCGI | library worker (`run_fastcgi_worker`); see `demos/nginx-fastcgi.conf` |

## Project Layout

```
Makefile       # Root orchestrator
.make/         # Build, test, quality, Elara discovery
elara_fpm/    # Host Cargo package + elara-fpm binary
bin/           # Release binary (git-ignored except .gitkeep)
demos/         # Sample .ela scripts and nginx snippets
docs/          # Public guides (EN + PT)
```

## Development

```bash
make build             # Release-build and copy binary to bin/elara-fpm
make test              # Run cargo test
make quality           # Format, clippy, and test
make install           # Install binary to ~/.local/bin
make install-system    # Install binary to /usr/local/bin
make uninstall         # Remove from ~/.local/bin
make uninstall-system  # Remove from /usr/local/bin
```

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.
