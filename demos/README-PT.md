# Demos do Elara FPM

Rode com o servidor embutido em texto claro a partir da raiz do repositório (depois de `make build`):

```bash
make build
make run
# ou: ./bin/elara-fpm -S 127.0.0.1:8080 -t demos
```

Depois abra `http://127.0.0.1:8080/` (ou `/hello.ela`).

| Caminho | Propósito |
| --- | --- |
| `hello.ela` | Escreve um corpo hello simples |
| `echo.ela` | Ecoa method, path, query, `req.args`, host/scheme/remote e body bruto |
| `form.ela` | Lê `name` de um POST urlencoded |
| `cookie.ela` | Define e depois lê um cookie de demo |
| `session.ela` | Incrementa um contador int de sessão (e um bool) entre requests |
| `redirect.ela` | Redireciona para `hello.ela` |
| `template.ela` | `res.render` com escape, `if` / `for` e include em `templates/` |
| `templates/` | Arquivos de template (não servidos como URL — espere `404`) |
| `static-index/` | Diretório só com `index.html` (sem script de índice) |
| `hi.elac` | Amostra de bytecode pré-compilada (corpo `elac-ok`); fonte equivalente `res.write("elac-ok\n")` com o wrapper do host |
| `style.css` | CSS estático ao lado dos scripts |
| `index.ela` | Índice do docroot com links para as demos |

## Access log

Cada request concluído em `-S`, CGI ou FastCGI imprime uma linha em stderr:

```text
<method> <path> <status> <elapsed_ms>
```

Exemplo: `GET /hello.ela 200 3`. O path omite `?` e a query string.

## HEAD

`HEAD` em qualquer path de demo bate com o status e headers de um `GET`, com body vazio (scripts ainda executam).

## Framing de request (`-S`)

- Com `Content-Length`, `-S` lê o body completo — experimente `POST` em `/echo.ela`.
- Keep-alive HTTP/1.1 reutiliza a conexão a menos que o cliente envie `Connection: close`.
- `Transfer-Encoding: chunked` não é implementado (`501`). Bodies acima do limite são `413`.

## FastCGI (`-F`)

Não combine `-S` e `-F`. Suba um worker neste docroot e aponte o nginx para ele (veja `nginx-fastcgi.conf`):

```bash
./bin/elara-fpm -F 127.0.0.1:9000 -t demos
# ou: ./bin/elara-fpm -F /tmp/elara-fpm.sock -t demos
```

CGI sem `-S`/`-F` continua válido; veja `nginx-cgi.conf`.

## Flags do operador

Aplicam-se a `-S`, CGI e `-F` no processo inteiro (veja também `docs/deployment-PT.md`):

| Flag | Papel |
| --- | --- |
| `--gas <n>` | Limite de gas por request |
| `--mem <bytes>` | Limite de memória por request |
| `--max-body <bytes>` | Tamanho máximo do body |
| `--session-dir <path>` | Diretório do store de sessão |
| `--session-secure` | Define `Secure` no cookie de sessão |
| `--session-samesite Lax\|Strict\|None` | `SameSite` do cookie de sessão |
| `--allow-sys-env` | Concede `ELARA_CAP_SYS_ENV` |

Exemplo com limite apertado de body:

```bash
./bin/elara-fpm -S 127.0.0.1:8080 -t demos --max-body 1024
```

Inglês: [`README.md`](README.md).
