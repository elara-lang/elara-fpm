# Elara HTTP

HTTP SAPI host that embeds the Elara VM through `elara.h` / `libelara`, for running `.ela` scripts behind a front-end web server (CGI / FastCGI) or locally with a built-in development listener.

## Highlights

- Keep HTTP and sockets in this host only — never inside the Elara language core
- Discover Elara from a sibling checkout or an install prefix via `ELARA_ROOT` / `ELARA_PREFIX`
- Link `libelara` and smoke-test the C ABI from the host crate
- Grow toward CGI, FastCGI, and a PHP-like `-S` local server on a shared request lifecycle

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

## Quick Start

```bash
make discover
make test
```

The link smoke opens an `elara_State`, evaluates `return 1+1`, and expects `2`.

## Usage

Host adapters (CGI, FastCGI, `-S`) land as the project grows. Until then, use `make test` / `make quality` to validate the Elara link.

## Development

```bash
make help       # targets + Elara env docs
make discover   # print resolved Elara paths
make check      # cargo check
make test       # cargo test (needs libelara on LD_LIBRARY_PATH via make)
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
| `bin/` | Built artifacts (git-ignored except `.gitkeep`) |
| `elara_http/` | Host Cargo package |

## License

MIT
