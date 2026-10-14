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

## Limites

- Só texto claro — sem TLS, sem HTTP/2
- Para desenvolvimento local, não produção pública
- Cada request recebe um estado de VM sandboxed novo
- Scripts `.ela` executam; outros arquivos no docroot são estáticos
- Caminhos não podem escapar do document root (`..` é rejeitado)

## Demos

Veja [`demos/README.md`](../demos/README.md).
