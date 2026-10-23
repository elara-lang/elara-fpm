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

## Flags do operador

Aplicam-se a `-S`, CGI e `-F` no processo inteiro. Scripts não as alteram.

| Flag | Padrão | Significado |
| --- | --- | --- |
| `--gas <n>` | `10000000` | Limite de gas por request (≥ 1) |
| `--mem <bytes>` | `33554432` | Limite de memória por request (≥ 1) |
| `--max-body <bytes>` | `1048576` | Tamanho máximo do body (≥ 0) |
| `--session-dir <path>` | `$TMPDIR/elara/.sessions` | Diretório do store de sessão |
| `--session-secure` | off | Define `Secure` no cookie de sessão |
| `--session-samesite <Lax\|Strict\|None>` | `Lax` | `SameSite` do cookie de sessão |
| `--allow-sys-env` | off | Concede `ELARA_CAP_SYS_ENV` |

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
