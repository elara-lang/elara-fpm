# Servidor local de desenvolvimento (`-S`)

```bash
make build
./bin/elara-fpm -S 127.0.0.1:8080 -t demos
```

| Flag | Significado |
| --- | --- |
| `-S <addr>:<port>` | Escuta HTTP/1.1 em texto claro |
| `-t <dir>` | Document root (padrão: diretório atual) |

Ao iniciar, o processo imprime a URL e o document root em **stderr**.

## Framing de request

- Com `Content-Length`, `-S` lê o body completo (em várias leituras do socket) até o limite do host (padrão 1 MiB). Acima disso responde HTTP `413` e fecha a conexão.
- Sem `Content-Length`, o body é vazio (inclusive em `POST`).
- `Transfer-Encoding: chunked` não é implementado; o servidor responde `501` e fecha.
- Keep-alive HTTP/1.1: se o cliente omite `Connection: close`, a conexão permanece aberta para o próximo request. Bytes restantes após um request iniciam o seguinte. HTTP/1.0 ou `Connection: close` encerra após a resposta.

## Limites

- Só texto claro — sem TLS, sem HTTP/2
- Para desenvolvimento local, não produção pública
- Cada request recebe um estado de VM sandboxed novo
- Scripts `.ela` executam; outros arquivos no docroot são estáticos
- Caminhos não podem escapar do document root (`..` é rejeitado)

## Demos

Veja [`demos/README.md`](../demos/README.md).
