# Bearer authentication through a standard HTTP policy

[日本語](README.md)

The standard HTTP dispatcher calls a Rust verifier and passes an authenticated `AuthScope` only to the `/me` handler. `/health` and `/restricted` are explicitly public routes.

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
| `/me`, missing or incorrect header | The same 401, `invalid credential`, and `WWW-Authenticate: Bearer` |
| `/me`, matching configured value | 200, `Hello, Nagi!` |
| `/me`, duplicate header names or invalid UTF-8 | 400, `invalid security request` |
| `/restricted` | A business 403 from the public handler, `access denied` |

The 401/400 responses are standard policy failures. `/restricted` is a handler response, not an authentication-policy denial. An empty `NAGI_DEMO_AUTHORIZATION` causes a startup error. Change the port with `NAGI_SAMPLE_PORT`; stop with Ctrl+C.

The `native.rs` verifier compares one configured string with a demo credential and returns a finite-lived `VerifiedIdentity` for subject 1. This demonstrates the boundary wiring; it does not implement a production token format, signature, audience, expiry validation, revocation lookup, or user management. Production use needs a reviewed token verifier.
