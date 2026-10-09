# Deployment (CGI / FastCGI)

## CGI

Run without `-S`. The process reads the CGI environment and stdin, then writes a CGI response to stdout:

```bash
elara-http -t /var/www/app
```

Typical env vars: `REQUEST_METHOD`, `PATH_INFO`, `QUERY_STRING`, `CONTENT_LENGTH`, `HTTP_*`.

Sample nginx snippet: [`demos/nginx-cgi.conf`](../demos/nginx-cgi.conf).

## FastCGI

Long-running worker that accepts FastCGI connections and reuses the process while keeping one isolated VM state per request. Sample nginx snippet: [`demos/nginx-fastcgi.conf`](../demos/nginx-fastcgi.conf).

## Document root

All script and static paths are resolved under the configured document root. Escape attempts (`..`, absolute paths) fail.

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
