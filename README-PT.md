# Elara FPM

Host SAPI HTTP que embute a VM Elara via `elara.h` / `libelara`, rodando scripts `.ela` atrás de CGI/FastCGI ou de um servidor de desenvolvimento em texto claro estilo PHP (`-S`).

## Destaques

- Manter HTTP e sockets só neste host — nunca no núcleo da linguagem Elara
- Descobrir a Elara em `/usr/local`, noutro prefixo (`ELARA_PREFIX`) ou num checkout irmão (`ELARA_ROOT`)
- Servidor embutido `-S` / `-t` para HTTP local em texto claro
- Modo CGI (sem `-S`) e worker FastCGI para deploy atrás de um front-end
- API de script: `req` / `res` (method, path, query, args, host/scheme/remote, body), forms, cookies, sessions, redirects, escape HTML
- Templates via `res.render` (escape, `if` / `for`, includes; `--template-dir`)
- Serve `.ela` / `.elac`, `index.ela` / `index.elac` / `index.html` em diretórios, e arquivos estáticos; `HEAD` bate com `GET` com body vazio
- Access log em stderr (`method path status elapsed_ms`) para `-S`, CGI e FastCGI
- Flags de operador: `--gas`, `--mem`, `--max-body`, `--session-dir`, `--session-secure`, `--session-samesite`, `--allow-sys-env`
- Sessões em arquivo sob `$TMPDIR/elara/.sessions` (tipicamente `/tmp/elara/.sessions`)

## Pré-requisitos

- **Rust 1.85+** (`cargo`, `rustc`, `rustfmt`, `clippy`) — [rustup](https://rustup.rs/)
- **Elara** compilada ou instalada com `elara.h` e `libelara` disponíveis (veja abaixo)

## Instalação

### Build from Source

```bash
git clone https://github.com/carlosrabelo/elara-fpm.git
cd elara-fpm
make build
```

Instale em `~/.local/bin` (padrão), ou em todo o sistema em `/usr/local/bin` (sudo só para a cópia):

```bash
make install
make install-system
make uninstall
make uninstall-system
```

### Dependência Elara

O Elara FPM não incorpora a árvore da linguagem. O build resolve headers e bibliotecas nesta ordem:

| Modo | Quando | Headers | Biblioteca |
| --- | --- | --- | --- |
| Install | `/usr/local` contém a instalação, ou `ELARA_PREFIX` está definido | `$prefix/include/elara.h` | `$prefix/lib/libelara.*` |
| Sibling | `ELARA_ROOT` está definido, ou `../elara` quando não há instalação | `$ELARA_ROOT/elara/include/elara.h` | `$ELARA_ROOT/bin` ou `elara/target/release` |

Sem nenhuma das variáveis, uma instalação em `/usr/local` é usada quando existem `include/elara.h` e `lib/libelara.*`; caso contrário usa-se o checkout irmão. Se `ELARA_PREFIX` e `ELARA_ROOT` estiverem definidos, o **modo install vence**. `ELARA_ROOT` seleciona o checkout mesmo quando `/usr/local` também tem a Elara.

```bash
# Padrão: /usr/local quando a Elara está instalada lá
make discover

# Prefixo explícito
ELARA_PREFIX=/opt/elara make discover

# Checkout irmão (vence /usr/local)
ELARA_ROOT=/caminho/para/elara make discover
```

Elara ausente termina com exit ≠ 0 e menciona `ELARA_ROOT` / `ELARA_PREFIX`.

Compile a Elara antes, se preciso:

```bash
cd ../elara && make build
```

## Início Rápido

```bash
make discover
make build
make run
```

Abra [http://127.0.0.1:8080/](http://127.0.0.1:8080/) para o hub de demos (ou `/hello.ela`). Mais demos: [`demos/README.md`](demos/README.md). Guias: [`docs/`](docs/). Inglês: [`README.md`](README.md).

## Uso

### Servidor de desenvolvimento local

```bash
make run
# sobrescrever endereço / docroot
make run ARGS='-S 127.0.0.1:9090 -t demos'
# ou chamar o binário diretamente
./bin/elara-fpm -S 127.0.0.1:8080 -t demos
```

### CGI

Sem `-S`, o binário fala CGI (environment + stdin → stdout):

```bash
./bin/elara-fpm -t demos
```

Config de front-end de exemplo: [`demos/nginx-cgi.conf`](demos/nginx-cgi.conf).

### FastCGI

Worker de longa duração via `-F` (TCP ou socket Unix). Não combine com `-S`. Nginx de exemplo: [`demos/nginx-fastcgi.conf`](demos/nginx-fastcgi.conf).

```bash
./bin/elara-fpm -F 127.0.0.1:9000 -t demos
./bin/elara-fpm -F /run/elara-fpm.sock -t demos
```

| Modo | Como |
| --- | --- |
| Local texto claro | `elara-fpm -S 127.0.0.1:8080 -t <docroot>` |
| CGI | `elara-fpm -t <docroot>` com env CGI + stdin (sem `-S` / `-F`) |
| FastCGI | `elara-fpm -F 127.0.0.1:9000 -t <docroot>` ou `-F /caminho/para.sock` |

## Estrutura do Projeto

```
Makefile       # Orquestrador raiz
.make/         # Build, test, quality, descoberta Elara
elara_fpm/    # Pacote Cargo do host + binário elara-fpm
bin/           # Binário de release (git-ignored exceto .gitkeep)
demos/         # Scripts .ela de exemplo e snippets nginx
docs/          # Guias públicos (EN + PT)
```

## Desenvolvimento

```bash
make build             # Release-build and copy binary to bin/elara-fpm
make test              # Run cargo test
make quality           # Format, clippy, and test
make install           # Install binary to ~/.local/bin
make install-system    # Install binary to /usr/local/bin
make uninstall         # Remove from ~/.local/bin
make uninstall-system  # Remove from /usr/local/bin
```

## Licença

Este projeto é licenciado sob a Licença MIT — consulte o arquivo [LICENSE](LICENSE) para mais detalhes.
