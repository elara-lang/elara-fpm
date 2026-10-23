# Deployment (CGI / FastCGI)

## CGI

Run without `-S`. The process reads the CGI environment and stdin, then writes a CGI response to stdout:

```bash
elara-fpm -t /var/www/app
```

Typical env vars: `REQUEST_METHOD`, `PATH_INFO`, `QUERY_STRING`, `CONTENT_LENGTH`, `HTTP_*`.

Sample nginx snippet: [`demos/nginx-cgi.conf`](../demos/nginx-cgi.conf).

## FastCGI

Long-running worker via `-F`. Listen on TCP (`addr:port`) or an absolute Unix socket path. Keep one isolated VM state per request. Do not pass `-S` together with `-F`.

```bash
elara-fpm -F 127.0.0.1:9000 -t /var/www/app
elara-fpm -F /run/elara-fpm.sock -t /var/www/app
```

Sample nginx snippet: [`demos/nginx-fastcgi.conf`](../demos/nginx-fastcgi.conf).

## Document root

All script and static paths are resolved under the configured document root. Escape attempts (`..`, absolute paths) fail.

## Operator flags

These apply to `-S`, CGI, and `-F` for the whole process. Scripts cannot change them.

| Flag | Default | Meaning |
| --- | --- | --- |
| `--gas <n>` | `10000000` | Per-request gas limit (≥ 1) |
| `--mem <bytes>` | `33554432` | Per-request memory limit (≥ 1) |
| `--max-body <bytes>` | `1048576` | Maximum request body size (≥ 0) |
| `--session-dir <path>` | `$TMPDIR/elara/.sessions` | Session file-store directory |
| `--session-secure` | off | Set `Secure` on the session cookie |
| `--session-samesite <Lax\|Strict\|None>` | `Lax` | Session cookie `SameSite` |
| `--allow-sys-env` | off | Grant `ELARA_CAP_SYS_ENV` |

## Sandbox defaults

| Knob | Default |
| --- | --- |
| Gas limit | Finite (10_000_000) |
| Memory limit | Finite (32 MiB) |
| `ELARA_CAP_SYS_ENV` | Off |

Operators may raise limits per process. Scripts cannot enable env access unless the host grants the capability.

## Security notes

- Terminate TLS on the front-end web server
- Keep the session store directory (`$TMPDIR/elara/.sessions`, typically `/tmp/elara/.sessions`) unreadable by other tenants
- Upload size limits apply before scripts run
- Do not expose `-S` to the public internet
