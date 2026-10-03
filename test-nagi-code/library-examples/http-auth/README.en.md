# Read headers and handle authentication errors in Nagi

A small Bearer authentication example using only the standard HTTP API. It needs no Rust adapter or database. Shared state holds a demo value, and an async handler reads `Authorization`.

Run from this directory:

```sh
export NAGI_DEMO_AUTHORIZATION='Bearer example-only-token'
nagic run
```

In PowerShell:

```powershell
$env:NAGI_DEMO_AUTHORIZATION = 'Bearer example-only-token'
nagic run
```

Try these in another terminal. Use `curl.exe` on Windows if needed.

```sh
curl -i http://127.0.0.1:8089/health
curl -i http://127.0.0.1:8089/me
curl -i -H 'Authorization: Bearer example-only-token' http://127.0.0.1:8089/me
curl -i http://127.0.0.1:8089/restricted
```

| Route | Response |
|---|---|
| `/health` | 200, `ok` |
| `/me`, missing or incorrect header | 401 |
| `/me`, matching configured value | 200, `Hello, Nagi!` |
| `/me`, duplicate headers or invalid UTF-8 | 400 |
| `/restricted` | Its route mapper overrides the app mapper and returns 403 |

401 responses include `WWW-Authenticate: Bearer` and omit the credential and internal errors. An unset value causes a startup error. Change the port with `NAGI_SAMPLE_PORT`; stop with Ctrl+C.

The fixed comparison demonstrates the HTTP API. This example does not implement production authentication, user management, or token issuance and revocation.

[日本語](README.md)
