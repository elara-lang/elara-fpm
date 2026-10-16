# Deploy (CGI / FastCGI)

## CGI

Execute sem `-S`. O processo lê o ambiente CGI e o stdin, e escreve a resposta CGI no stdout:

```bash
elara-fpm -t /var/www/app
```

Env típicas: `REQUEST_METHOD`, `PATH_INFO`, `QUERY_STRING`, `CONTENT_LENGTH`, `HTTP_*`.

Snippet nginx: [`demos/nginx-cgi.conf`](../demos/nginx-cgi.conf).

## FastCGI

Worker de longa duração via `-F`. Escute em TCP (`addr:port`) ou em um caminho absoluto de socket Unix. Mantém um estado de VM isolado por request. Não use `-S` junto com `-F`.

```bash
elara-fpm -F 127.0.0.1:9000 -t /var/www/app
elara-fpm -F /run/elara-fpm.sock -t /var/www/app
```

Snippet nginx: [`demos/nginx-fastcgi.conf`](../demos/nginx-fastcgi.conf).

## Document root

Todos os caminhos de script e estáticos resolvem sob o document root configurado. Tentativas de escape (`..`, caminhos absolutos) falham.

## Padrões de sandbox

| Controle | Padrão |
| --- | --- |
| Limite de gas | Finito (10_000_000) |
| Limite de memória | Finito (32 MiB) |
| `ELARA_CAP_SYS_ENV` | Desligado |

Operadores podem aumentar limites por processo. Scripts não acessam env a menos que o host conceda a capability.

## Notas de segurança

- Termine TLS no servidor web de front-end
- Mantenha o diretório de sessões (`$TMPDIR/elara/.sessions`, tipicamente `/tmp/elara/.sessions`) inacessível a outros tenants
- Limites de upload aplicam antes do script
- Não exponha `-S` à internet pública
