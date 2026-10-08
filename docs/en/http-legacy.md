# Migrating retired HTTP APIs

Unreleased 0.2.0 SF01 removes `@get/@post/@put/@delete`, `serve(Db, port)`, and `Html/html`. High, independently saved Low, and handwritten Low reject them with a checker `SF01 migration` diagnostic. There is no legacy coexistence or policy-free escape hatch.

Consult the repository at the published 0.1.x version for its historical behavior. In development source, start with [HTTP](http.md) and the [0.2.0 migration guide](migration-0.2.0.md) for explicit Policy, handlers, path/query, JSON/body and error mapping.

Register `/health` as an explicit public route. `/stream` and `/ws` are not injected. Complete bounded bytes responses are available; arbitrary streaming, WebSocket and active HTML remain unimplemented. Typed HTML is a separate SF04 step, and equivalent migration of old HTML screens is still outstanding.
