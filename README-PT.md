# Elara HTTP

Host SAPI HTTP que embute a VM Elara via `elara.h` / `libelara`, rodando scripts `.ela` atrás de CGI/FastCGI ou de um servidor de desenvolvimento em texto claro estilo PHP (`-S`).

## Destaques

- Manter HTTP e sockets só neste host — nunca no núcleo da linguagem Elara
- Descobrir a Elara em `/usr/local`, noutro prefixo (`ELARA_PREFIX`) ou num checkout irmão (`ELARA_ROOT`)
- Servidor embutido `-S` / `-t` para HTTP local em texto claro
- Modo CGI (sem `-S`) e worker FastCGI para deploy atrás de um front-end
- API de script: `req` / `res`, forms, cookies, sessions, redirects, escape HTML
- Sessões em arquivo sob `$TMPDIR/elara/.sessions` (tipicamente `/tmp/elara/.sessions`)

## Pré-requisitos

- **Rust 1.85+** (`cargo`, `rustc`, `rustfmt`, `clippy`) — [rustup](https://rustup.rs/)
- **Elara** compilada ou instalada com `elara.h` e `libelara` disponíveis (veja abaixo)

## Instalação

### Build from Source

```bash
git clone https://github.com/carlosrabelo/elara-http.git
cd elara-http
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

O Elara HTTP não incorpora a árvore da linguagem. O build resolve headers e bibliotecas nesta ordem:

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

Abra [http://127.0.0.1:8080/hello.ela](http://127.0.0.1:8080/hello.ela). Mais demos: [`demos/README.md`](demos/README.md). Guias: [`docs/`](docs/). Inglês: [`README.md`](README.md).

## Uso

### Servidor de desenvolvimento local

```bash
make run
# sobrescrever endereço / docroot
make run ARGS='-S 127.0.0.1:9090 -t demos'
# ou chamar o binário diretamente
./bin/elara-http -S 127.0.0.1:8080 -t demos
```

### CGI

Sem `-S`, o binário fala CGI (environment + stdin → stdout):

```bash
./bin/elara-http -t demos
```

Config de front-end de exemplo: [`demos/nginx-cgi.conf`](demos/nginx-cgi.conf).

### FastCGI

Use o worker FastCGI da biblioteca atrás de um front-end. Config de exemplo: [`demos/nginx-fastcgi.conf`](demos/nginx-fastcgi.conf).

| Modo | Como |
| --- | --- |
| Local texto claro | `elara-http -S 127.0.0.1:8080 -t <docroot>` |
| CGI | `elara-http -t <docroot>` com env CGI + stdin (sem `-S`) |
| FastCGI | worker da biblioteca (`run_fastcgi_worker`); ver `demos/nginx-fastcgi.conf` |

## Estrutura do Projeto

```
Makefile       # Orquestrador raiz
.make/         # Build, test, quality, descoberta Elara
elara_http/    # Pacote Cargo do host + binário elara-http
bin/           # Binário de release (git-ignored exceto .gitkeep)
demos/         # Scripts .ela de exemplo e snippets nginx
docs/          # Guias públicos (EN + PT)
```

## Desenvolvimento

```bash
make build             # Release-build and copy binary to bin/elara-http
make test              # Run cargo test
make quality           # Format, clippy, and test
make install           # Install binary to ~/.local/bin
make install-system    # Install binary to /usr/local/bin
make uninstall         # Remove from ~/.local/bin
make uninstall-system  # Remove from /usr/local/bin
```

## Licença

Este projeto é licenciado sob a Licença MIT — consulte o arquivo [LICENSE](LICENSE) para mais detalhes.
