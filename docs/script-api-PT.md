# Visão geral da API de script

Scripts recebem `req` e `res` (e opcionalmente `session`) a cada request.

## `req`

| Campo | Significado |
| --- | --- |
| `method` | Método HTTP em maiúsculas |
| `path` | Caminho do request |
| `query` | Query string bruta (sem `?`) |
| `args` | Mapa da query parseada (percent-decode; `+` é espaço; **última chave vence**) |
| `headers` | Mapa de headers em minúsculas |
| `body` | Corpo bruto |
| `host` | Host do request (`Host` / `HTTP_HOST` / `SERVER_NAME`); pode incluir `:port`; vazio se ausente |
| `scheme` | `http` ou `https` (`-S` é sempre `http`) |
| `remote` | Endereço do cliente sem porta (peer / `REMOTE_ADDR`); vazio se ausente |
| `form` | Campos urlencoded / multipart texto (só do body; não mesclado com `args`) |
| `cookies` | Header `Cookie` parseado |
| `json` | Objeto JSON parseado (ou `nil`) |
| `files` | Uploads multipart (`name`, `filename`, `content_type`, `path`, `size`) |

O host **não** confia em `X-Forwarded-Proto` nem `X-Forwarded-For` para `scheme` / `remote`. Deixe TLS e a identidade confiável do cliente no proxy de front-end.

## `res`

| Chamada | Efeito |
| --- | --- |
| `res.set_status(code)` | Status HTTP |
| `res.header(name, value)` | Header de resposta |
| `res.write(chunk)` | Acrescenta texto ao corpo |
| `res.cookie(name, value)` | `Set-Cookie` com padrões |
| `res.cookie_ex(...)` | Cookie com path / flags / max-age |
| `res.redirect(url)` | 302 + `Location` (status opcional 301/303/307/308) |

Escritas após `redirect` são ignoradas.

## Helpers

- `html_escape(s)` — escapa `& < > " '` para texto/atributos HTML
- `session.get(key)` / `session.set(key, value)` / `session.destroy()` — sessão em arquivo (quando o host habilita). Valores persistidos podem ser string, int, bool, nil ou tables desses tipos (tables aninhadas até profundidade 8). Floats, functions, userdata e tables mais profundas falham em `session.set` sem alterar o arquivo de sessão anterior.

## Exemplo

```ela
res.header("Content-Type", "text/plain; charset=utf-8")
res.write("hello ")
res.write(req.method)
```

Veja também [`demos/`](../demos/).
