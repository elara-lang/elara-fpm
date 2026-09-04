# Elara HTTP

Host SAPI HTTP que embute a VM Elara via `elara.h` / `libelara`, para rodar scripts `.ela` atrás de um servidor web (CGI / FastCGI) ou localmente com um listener de desenvolvimento embutido.

## Destaques

- Manter HTTP e sockets só neste host — nunca no núcleo da linguagem Elara
- Descobrir a Elara a partir de um checkout irmão ou de um prefixo de instalação via `ELARA_ROOT` / `ELARA_PREFIX`
- Linkar `libelara` e validar a ABI C a partir do crate do host
- Evoluir para CGI, FastCGI e um servidor local estilo `php -S` com o mesmo ciclo de request

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

## Início rápido

```bash
make discover
make test
```

O smoke de link abre um `elara_State`, avalia `return 1+1` e espera `2`.

## Uso

Os adapters do host (CGI, FastCGI, `-S`) entram conforme o projeto avança. Até lá, use `make test` / `make quality` para validar o link com a Elara.

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
| `Makefile` | Orquestrador na raiz |
| `.make/` | Build, teste, quality, descoberta da Elara |
| `bin/` | Artefatos (ignorados no git, exceto `.gitkeep`) |
| `elara_http/` | Pacote Cargo do host |

## Licença

MIT
