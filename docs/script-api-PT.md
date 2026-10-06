# Visão geral da API de script

Scripts recebem `req` e `res` (e opcionalmente `session`) a cada request.

## `req`

| Campo | Significado |
| --- | --- |
| `method` | Método HTTP em maiúsculas |
| `path` | Caminho do request |
| `query` | Query string bruta (sem `?`) |
| `headers` | Mapa de headers em minúsculas |
| `body` | Corpo bruto |
| `form` | Campos urlencoded / multipart texto |
| `cookies` | Header `Cookie` parseado |
| `json` | Objeto JSON parseado (ou `nil`) |
| `files` | Uploads multipart (`name`, `filename`, `content_type`, `path`, `size`) |

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
- `session.get(key)` / `session.set(key, value)` / `session.destroy()` — sessão em arquivo (quando o host habilita)

## Exemplo

```ela
res.header("Content-Type", "text/plain; charset=utf-8")
res.write("hello ")
res.write(req.method)
```

Veja também [`demos/`](../demos/).
