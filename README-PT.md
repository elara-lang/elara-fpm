# Elara HTTP

Host **SAPI HTTP** que embute a VM Elara via `elara.h` / `libelara`. Roda scripts `.ela` atrás de um front-end (CGI / FastCGI) ou localmente com um servidor de desenvolvimento em texto claro estilo PHP (`-S`). Este host **não** é um servidor TLS/HTTP/2 de produção — termine TLS no nginx, Caddy ou Apache.

## Destaques

- Manter HTTP e sockets só neste host — nunca no núcleo da linguagem Elara
- Descobrir a Elara a partir de um checkout irmão ou de um prefixo via `ELARA_ROOT` / `ELARA_PREFIX`
- Servidor embutido `-S` / `-t` para HTTP local em texto claro
- Modo CGI (sem `-S`) e worker FastCGI para deploy atrás de um front-end
- API de script: `req` / `res`, forms, cookies, sessions, redirects, escape HTML

## Pré-requisitos

- **Rust 1.85+** (`cargo`, `rustc`, `rustfmt`, `clippy`) — [rustup](https://rustup.rs/)
- **Elara** compilada ou instalada com `elara.h` e `libelara` disponíveis (veja abaixo)

## Instalação

### Compilar este repositório

```bash
git clone https://github.com/carlosrabelo/elara-http.git
cd elara-http
make build
make test
```

### Dependência Elara

O Elara HTTP não incorpora a árvore da linguagem. O build resolve headers e bibliotecas nesta ordem:

| Modo | Quando | Headers | Biblioteca |
| --- | --- | --- | --- |
| Install | `ELARA_PREFIX` definido | `$ELARA_PREFIX/include/elara.h` | `$ELARA_PREFIX/lib/libelara.*` |
| Sibling | `ELARA_ROOT` ou `../elara` | `$ELARA_ROOT/elara/include/elara.h` | `$ELARA_ROOT/bin` ou `elara/target/release` |

Se `ELARA_PREFIX` e `ELARA_ROOT` estiverem definidos, o **modo install vence**.

```bash
# Sibling (padrão quando ../elara existe)
make discover

# Root irmão explícito
ELARA_ROOT=/caminho/para/elara make discover

# Prefixo instalado
ELARA_PREFIX=/usr/local make discover
```

Elara ausente termina com exit ≠ 0 e menciona `ELARA_ROOT` / `ELARA_PREFIX`.

Compile a Elara antes, se preciso:

```bash
cd ../elara && make build
```

## Início rápido (`-S`)

```bash
make discover
make build
cargo run --manifest-path elara_http/Cargo.toml --bin elara-http -- -S 127.0.0.1:8080 -t demos
```

Abra [http://127.0.0.1:8080/hello.ela](http://127.0.0.1:8080/hello.ela). Mais demos: [`demos/README.md`](demos/README.md).

Guias: [`docs/`](docs/) (API de script, servidor local, deploy). Inglês: [`README.md`](README.md).

## Uso

| Modo | Como |
| --- | --- |
| Local texto claro | `elara-http -S 127.0.0.1:8080 -t <docroot>` |
| CGI | `elara-http -t <docroot>` com env CGI + stdin (sem `-S`) |
| FastCGI | worker da biblioteca (`run_fastcgi_worker`); ver `demos/nginx-fastcgi.conf` |

## Desenvolvimento

```bash
make help       # alvos + docs das env da Elara
make discover   # imprime caminhos resolvidos
make check      # cargo check
make test       # cargo test (precisa de libelara via make)
make lint       # clippy -D warnings
make fmt        # rustfmt
make quality    # fmt check + clippy + test
make clean
```

## Layout do projeto

| Caminho | Papel |
| --- | --- |
| `Makefile` | Orquestrador raiz |
| `.make/` | Build, test, quality, descoberta Elara |
| `elara_http/` | Pacote Cargo do host + binário `elara-http` |
| `demos/` | Scripts `.ela` de exemplo e snippets de front-end |
| `docs/` | Guias públicos (EN + PT) |

## Licença

MIT
