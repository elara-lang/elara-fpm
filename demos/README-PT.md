# Demos do Elara HTTP

Rode com o servidor embutido em texto claro a partir da raiz do repositório (depois de `make build`):

```bash
make build
make run
# ou: ./bin/elara-http -S 127.0.0.1:8080 -t demos
```

Depois abra `http://127.0.0.1:8080/hello.ela`.

| Script | Propósito |
| --- | --- |
| `hello.ela` | Escreve um corpo hello simples |
| `echo.ela` | Ecoa method, path, query e body bruto |
| `form.ela` | Lê `name` de um POST urlencoded |
| `cookie.ela` | Define e depois lê um cookie de demo |
| `session.ela` | Incrementa um contador de sessão entre requests |
| `redirect.ela` | Redireciona para `hello.ela` |
| `style.css` | CSS estático ao lado dos scripts |
| `index.ela` | Índice do docroot com links para as demos |

Exemplos de deploy no front-end: veja `nginx-cgi.conf` e `nginx-fastcgi.conf`.

Inglês: [`README.md`](README.md).
