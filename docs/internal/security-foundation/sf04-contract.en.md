# SF04 typed HTML: public contract (frozen; in development)

[日本語契約](sf04-contract.md) · [Implementation/RED map](sf04-implementation-map.en.md) · [Adopted RFC](rfc.en.md) · [D1–D3](decisions-and-migration.en.md)

2026-10-10 UTC. **SF04-C01–C08 are frozen as concrete implementation contracts within the adopted RFC. APIs/numbers remain as listed; the complete feature is not implemented/accepted.** Pinned actual main: `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`, tree `9172331188d0d39f6e91ca29c47e6ec9cf691ce5`. Comparison SF05: `47dfc09ab36e553e45f8b49b1b3eba5ee16ddc7b`. This excludes stale local main and SF05/reader/editor WIP. SF04 directly depends only on merged SF01 (SF04 row in the [plan](implementation-plan.en.md)); this document does not replace dependent-feature acceptance or required prior CI.

## Adopted boundaries and remaining decisions

The Japanese RFC's HTML section, lines 127–135, and D1 require opaque typed HTML, separate text/quoted-attribute/allowed-URL operations, static allowlists, preserved contexts, finite budgets, removal of raw entries, and finalizer-owned Content-Type/body kind/security headers. No arbitrary HTML sanitizer, string/bytes cast, JS/CSS/rawtext, handlers, srcdoc, unsupported namespace or unsafe-inline is provided. A navigation URL differs from server-side SSRF Target. CSP does not replace encoding or authorization.

The IDs below are RFC implementation choices frozen at6ef4d21 after independent review/R01 recheck; D1–D3 themselves do not require renewed approval.

| ID | Frozen concrete choice | Security, compatibility and decision |
|---|---|---|
| SF04-C01 | Thirteen operations, six resources, owned transforms, storage/shared for immutable data | Allow encoded data across tasks; it is not a proof. Check native Send/Sync and ordinary checker agreement for acceptance |
| SF04-C02 | Twelve inert tags, title/href only, closed content model | Small list/detail UI; not fully compatible with arbitrary HTML/forms/style/script |
| SF04-C03 | Five-character escaping, Unicode preservation, rejection of NUL/selected controls | Prevent reinterpretation; old UI expecting NUL replacement or normalization must migrate |
| SF04-C04 | Fixed HTTPS base and same-origin root-relative href only | Narrow navigation slots. No external/absolute links initially and no SSRF guarantee |
| SF04-C05 | Hard ceilings below, equal profiles, explicit bounded copy | Bound each value's retained/rendered content; no whole-app memory/CPU/cancellation-time cap |
| SF04-C06 | Private BodyKind, consuming typed document, CSP on all standard responses | Adds guarantees absent today. Review all-response CSP and HTML Content-Encoding compatibility |
| SF04-C07 | Development url 2.5.8 and small fixed-context encoder | Exact graph reviewed; remaining maintenance/advisory/distribution/target conditions are in the development decision |
| SF04-C08 | Ordinary Error, retained raw migration diagnostics, business-app/DOM oracle | Undefined stubs/parser failure are not feature RED success. Old raw coverage stays a separate commit |


Ordinary API names/subsets/numbers/resource flags are implementation choices within the accepted RFC, frozen through independent review. This contract freeze does not authorize new keywords/effects/independent checkers, Task/move/Grant/Tx changes, weakened guarantees or unadopted unchecked compatibility APIs. Any such separate public-semantic change needs its impact/alternatives returned for user decision.

## Resources and ownership — SF04-C01

Contract module: `std.html` (alias `html` here); native module: `::nagi_runtime::html`. All resources have generic arity zero, empty generic-role sets and empty inline_type_arguments. Except HtmlTag, they are opaque owned values with private fields. No public accessor exposes encoded strings, URL, policy or structural summary.

| Resource | Copy | Equality | Storage | Shared | Debug | Serde | Lifecycle |
|---|---|---|---|---|---|---|---|
| HtmlPolicy | false | false | true | true | false | false | Unspecified |
| HtmlAttributes | false | false | true | true | false | false | Unspecified |
| NavigationUrl | false | false | true | true | false | false | Unspecified |
| HtmlFragment | false | false | true | true | false | false | Unspecified |
| HtmlDocument | false | false | true | true | false | false | Unspecified |
| HtmlTag | true | true | true | true | true | false | Unspecified |

Storage/Shared use existing metadata semantics; they add no Task rule. Encoded data is not an authorization/authentication proof. The contract allows ordinary owned Task transfer and existing shared containers. SameTask AuthScope/Grant, Tx, move and cancellation contracts stay unchanged. Internal shared profiles contain no secret/Grant/Tx. Confirm immutable native Send/Sync and final Rust traits without deferring Nagi-detectable generated move/lifetime errors to rustc.

Only HtmlTag has native Copy/Clone. The other types have no public Clone/Default/From[str]/From[bytes]/Deserialize/Display/raw factory and reject Copy/Serde derivation. They reject Debug; existing Response Debug remains status/header_count/body_bytes only. Duplication uses the explicit bounded `copy_fragment` operation. Storage/shared permission to move a value does not grant Clone.

| Operation signature | Passing |
|---|---|
| `html.policy(base_origin: view[str]) -> Result[HtmlPolicy, Error]` | Reference |
| `html.limits(policy: HtmlPolicy, output_bytes: i64, input_bytes: i64, nodes: i64, depth: i64, urls: i64, url_bytes: i64) -> Result[HtmlPolicy, Error]` | Move × 7 |
| `html.empty(policy: view[HtmlPolicy]) -> HtmlFragment` | Reference |
| `html.text(policy: view[HtmlPolicy], value: view[str]) -> Result[HtmlFragment, Error]` | Reference, Reference |
| `html.attributes(policy: view[HtmlPolicy]) -> HtmlAttributes` | Reference |
| `html.title(attributes: HtmlAttributes, value: view[str]) -> Result[HtmlAttributes, Error]` | Move, Reference |
| `html.href(attributes: HtmlAttributes, value: NavigationUrl) -> Result[HtmlAttributes, Error]` | Move, Move |
| `html.navigation(policy: view[HtmlPolicy], value: view[str]) -> Result[NavigationUrl, Error]` | Reference, Reference |
| `html.element(policy: view[HtmlPolicy], tag: HtmlTag, attributes: HtmlAttributes, children: HtmlFragment) -> Result[HtmlFragment, Error]` | Reference, Move, Move, Move |
| `html.join(policy: view[HtmlPolicy], left: HtmlFragment, right: HtmlFragment) -> Result[HtmlFragment, Error]` | Reference, Move, Move |
| `html.copy_fragment(policy: view[HtmlPolicy], fragment: view[HtmlFragment]) -> Result[HtmlFragment, Error]` | Reference, Reference |
| `html.document(policy: view[HtmlPolicy], title: view[str], body: HtmlFragment) -> Result[HtmlDocument, Error]` | Reference, Reference, Move |
| `http.html_response(status: http.Status, document: html.HtmlDocument) -> http.Response` | Move, Move |

Every operation is synchronous, generic arity zero, `emit_type_args=false`, `borrow_owner=None`. Results own their data; no view result is returned. Explicit `view[...]` inputs use existing Passing::Reference; owned/scalar inputs use Move (HtmlTag is itself Copy). Caller views are retained only for the call. Internal immutable profile ownership does not extend a Reference input lifetime.

Standard Rust public signatures use the same types/order/results, mapping `view[T]` to `&T`, str to `&str`, i64 to i64 and Error to existing runtime Error. `http_server::html_response(Status, html::HtmlDocument) -> Response` is the sole contracted HTML response entry. Standard Rust also has no unchecked constructor, public fields/body-kind setter, unsafe conversion trait, or ambient string/bytes HTML response factory. Arbitrary active delivery from custom native Rust/unsafe extern belongs to a separate trusted-host author boundary; it is not documented as a standard-API workaround.

## Subset and structure — SF04-C02

HtmlTag constants: `DIV, SPAN, P, H1, H2, STRONG, EM, UL, OL, LI, A, BR` (twelve). Optional title occurs once on any user tag; optional href occurs once on A only. Duplicate attributes are Error::Invalid; serialization order is title then href. No dynamic names, id/class/data-/aria-, forms/images/media/table/iframe/template/object/embed/link/base/meta, CSS/JS/handlers/srcdoc, SVG or MathML are exposed. No operation takes an arbitrary name and sanitizes it later.

Flow is phrasing plus DIV/P/H1/H2/UL/OL; LI is allowed only as a list child. P/H1/H2/SPAN/STRONG/EM/A accept phrasing children only (text, BR, SPAN, STRONG, EM, A). An A anywhere under another A is rejected. DIV/LI accept flow/phrasing. UL/OL accept empty or LI-only sequences, excluding even whitespace text. Document body accepts flow/phrasing and rejects directly placed LI. BR accepts only empty children and serializes as a void element with no end tag. Closed summaries aim to avoid browser implicit closing/foster parenting; actual browser structure has not been tested.

`join` concatenates siblings; `element`/`document` check their children's summaries. An orphan LI can be held as an intermediate fragment for UL/OL; join assumes no parent. Private summaries track depth/count/anchor presence/top-level classes. HTML strings are not reparsed to recover a trusted type.

Only `document` creates a fixed wrapper. Internal head/meta/title are not public HtmlTag values. A plain str title is context-encoded into fixed RCDATA; body accepts only a typed fragment.

```html
<!doctype html><html><head><meta charset="utf-8"><title>{encoded title}</title></head><body>{typed fragment}</body></html>
```

## Text, quoted attributes and Unicode — SF04-C03

Text, title attribute and document title are distinct operations/fixed contexts. Escape five characters consistently: `&`→`&amp;`, `<`→`&lt;`, `>`→`&gt;`, `"`→`&quot;`, `'`→`&#39;`. Attributes use double quotes. Document title also escapes `&`/`<` so input cannot be reinterpreted as its terminator.

Preserve UTF-8 Unicode scalars without normalization. U+0000 is Invalid. Reject C0 and DEL except TAB/LF/CR; encode those three as `&#9;` / `&#10;` / `&#13;` to avoid browser newline normalization. No raw byte-to-str/fragment cast exists. Small inert Unicode/quote/ampersand fixtures check logical DOM text and quoted values.

Plain input `&amp;` becomes `&amp;amp;`, preserving literal text. Join/copy/element do not escape an already encoded fragment again. Validate/canonicalize URLs before quoted-attribute encoding. JSON encoding is not reused as HTML/JS encoding.

## NavigationUrl — SF04-C04/C07

`policy` parses a bounded raw HTTPS base origin and canonicalizes typed scheme/host/effective port. Reject userinfo including empty `@`, query/fragment, non-root path and a root dot. Canonicalize default ports. Check domain label/total length on canonical ASCII; use the parser's typed Host for IPv4/IPv6 literals. Do not compare raw origin suffixes/substrings.

`navigation` accepts only a single-leading-`/` root-relative path/query/fragment. Reject `//`, backslash, all C0/DEL/ASCII space before parsing. Absolute/scheme-bearing input is unsupported. Join with the fixed base using the parser, verify identical final scheme/typed host/effective port, and retain the intended canonical absolute URL. Recheck the final serialized path/query/fragment href itself for exactly one leading slash and reject canonical network-relative representations. Re-resolve that final href against the same fixed base and confirm identical origin (scheme/typed host/effective port) and intended canonical URL path/query/fragment, including absent versus empty values, before constructing NavigationUrl. A mismatch is Error::Invalid. Perform these final checks before HTML attribute encoding and retain output-budget checks after encoding. Separate URL percent encoding from HTML attribute encoding. Query/fragment are supported, but HTML element-id construction is initially absent.

This href constraint does not authorize server DNS/socket connections, provide SSRF special-address classification, authorize endpoints, or control all navigation through HTTP redirects/Location. Browser oracles must check rejection of post-normalization network-relative representations and equality of each accepted final href's browser destination to the intended canonical origin/path/query/fragment. No base element is generated; CSP `base-uri 'none'` is supplemental. Do not provide casts/shared unchecked wrappers between NavigationUrl and SSRF Target.

Base main/SF05 had no url/idna. At6ef4d21, [development adoption](sf04-url-adoption.en.md) selected exact URL2.5.8 default/std→IDNA1.1.0 compiled_data→ICU2.3. The reviewed lock adds25 with zero replacements, pins source/checksums/declared licenses, and reaches declared MSRV1.88 while only1.99 was executed. URL2.5.8 [parser](https://docs.rs/crate/url/2.5.8/source/src/parser.rs) trims C0/space, ignores tabs/newlines and reports EmbeddedCredentials even for empty userinfo; [idna](https://docs.rs/crate/idna/1.1.0/source/src/lib.rs) uses DnsLength::Ignore. Implementation retains strict raw/credential/length checks.

The old2.2.x support table establishes neither support nor unsupported status; Nagi maintainers own updates/current-policy tracking. Zero affected versions in the fixed RustSecDB metadata comparison is not formal audit/safety approval. Keep absent ICU4X_DATA_DIR, selected MIT/full Unicode/IBM notices, feature union/minimum toolchain/four-OS/browser/generated-app/CI acceptance conditions in the development decision.
- `url 2.5.8` archive SHA-256: `ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed`; [registry archive](https://static.crates.io/crates/url/url-2.5.8.crate).
- `idna 1.1.0` archive SHA-256: `3b0875f23caa03898994f6ddc501886a45c7d3d62d04d2d90788d47be1b1e4de`; [registry archive](https://static.crates.io/crates/idna/idna-1.1.0.crate).

## Finite budgets and profiles — SF04-C05

| Budget | Default / hard ceiling | Accounting |
|---|---|---|
| output_bytes | 65,536 | Encoded UTF-8, markup + document wrapper included |
| input_bytes | 16,384 | Aggregate raw UTF-8 text/title/attribute/URL payload |
| nodes | 1,024 | Text slots, elements, seven fixed document nodes |
| depth | 16 | Maximum structural nesting, document wrapper included |
| urls | 16 | Number of href slots |
| url_bytes | 2,048 | Each raw and canonical URL; base origin also bounded |
| attributes per element | 2 | Optional title and href only |

`limits` only lowers values below hard ceilings. output/input/nodes/depth/url_bytes must be positive; urls may be zero to disable links. Negative/excessive values are Invalid. The base origin is separately bounded by url_bytes at startup, without repeated charging to each fragment input. Every resource privately carries canonical base and all limits; mixing profiles in href/element/join/copy/document is Invalid. Equal profiles from different policy values count as identical. Composition cannot widen origins/budgets.

Sum raw text/title/URL input summaries, counting a copied fragment twice when joined. Attributes/NavigationUrl are bounded at creation; an element charges their summaries once. Raw and canonical URLs both fit url_bytes, while HTML-escaped bytes count toward output_bytes. Output is final UTF-8 bytes including title/markup/wrapper. Check arithmetic and encoded length before growing buffers.

A text slot counts as one node even when empty; empty fragment is zero; element is one plus children. Document adds seven fixed nodes (doctype/html/head/meta/title/title text/body) to body. Text depth is zero, BR one, element adds one, join takes max. Document depth is max(3, body depth + 2). Recheck complete output at document construction including wrapper cost. Avoid unbounded recursion/intermediate AST/vector independent of these limits.

These bound one value/response, not all stored values/tasks/shared data, allocator RSS, total request CPU or HTTP deadlines. Synchronous input work is bounded by the bytes/nodes above, without introducing forced cancellation during non-yielding work. Existing OOM/panic/Task cancellation boundaries remain.

## Response and finalizer — SF04-C06

Pinned main3b Response had only private status/headers/buffered Bytes. The C06 runtime slice implements private BodyKind `Empty/Text/Binary/Json/Html`, CSP and managed MIME; only `html_response` consumes a checked HtmlDocument and sets Html. Preserve existing fixed text/bytes/JSON MIME, nosniff, HEAD/204/205/304 and reserved-header rejection. Record actual unit outcomes within their bounded validation scope; independent C06 implementation review at ce2ba17 has no required findings.

The finalizer owns Html's `text/html; charset=utf-8`, existing MIME for other kinds, and nosniff on all responses. Manual headers cannot change bytes/JSON/text/mapper into HTML/JS/SVG. Future native compile-fail checks cover standard Rust unchecked factories/public fields/kind setters/conversions. This does not promise safety of content after download or arbitrary secrets.

Require this fixed CSP on every standard response (typed HTML, text/JSON/bytes, mapper, builtin denial/error, HEAD, 204/205/304). Nonces, reporting endpoints, unsafe-inline and script/style sources require separate contracts.

`default-src 'none'; script-src 'none'; style-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'`

Preserve current reserved Content-Type/CSP/nosniff/CORS/session fields. Additionally require managed Content-Encoding for HTML, rejected by append and rechecked by finalizer. Pinned main3b append_header did not reserve Content-Encoding: **this contract preserves existing nonHTML behavior**. Global rejection would require a separately recorded compatibility choice. No partial streaming/compressed-HTML factory exists.

Root froze the C06 internal-invariant handling as an ordinary implementation decision. If private state contains HTML Content-Encoding that the standard public API cannot construct, discard the old body/headers, substitute fixed `internal error`500text and pass it through the same managed finalizer. Do not echo values, add a public fallible API or globally reject nonHTML encoding. The consuming HtmlDocument bridge is crate-private, not a public raw reconstruction entry.

HEAD still constructs and checks the typed document before emptying body while retaining representation Content-Length. Preserve current 204/205/304 removal of body/Content-Type/Content-Encoding/framing and 205 Content-Length 0. Retain CSP/nosniff without a body under this contract. Do not add checker rules banning HTML status values.

Typed handler/mapper responses use the same finalizer; builtin early errors use safe text kind and the same managed headers. Invalid inputs/profiles/structure/budgets return existing Error::Invalid. Detected fallible allocation/internal failures return Error::Internal with bounded static messages and no input echo. No recovery from process OOM is promised. Ordinary Result Err is not Task Failure/panic/cancellation. Existing HTTP/Task panic, mapper panic, deadlines, cancellation and post-response-start limitations remain.

## Migration and equivalent business app — SF04-C08

Reject raw `Html`/`html`/`http.html` as required by D1, retaining current main's raw http.html rejection and canonical tombstones and existing diagnostic directing migration to SF04 typed APIs. Imports/aliases/function values/saved Low/manual Low/native replacements must not restore raw entry points. Calling undefined proposed APIs and observing unresolved/parser errors is not SF04 feature RED success.

Separate local raw-alias migration coverage `40877b3` is a test-only38-line change on another worktree, not integrated main. If needed later, explicitly cherry-pick and record its independent results/commit; do not duplicate it here.

Recommend an equivalent list/detail app through existing policy-required App: dynamic business title/description as text, optional quoted title, UL/LI list, typed A link `/items/7?view=details`, and typed document into `http.html_response`. Preserve existing public/protected routes, Grant checks/authorization, business status and Result mapping. The same High/saved-Low/handwritten-Low app must retain wire/browser DOM list/detail content and links. Plain text/JSON alone is not an equivalent HTML UI.

Old arbitrary pages requiring script/style/forms are not equivalent to this subset. Redesign as inert list/detail or choose a separate trusted-host author boundary; no unchecked compatibility shim is added. The business-app/DOM oracle remains future work; isolated runtime-core checks do not complete the app.

## Sources and validation boundary

Reuse both existing SF04 source/API-pattern inventories (two snapshots,36 hashes verified by the parent against Git objects). Main is the reference for citations; SQL changes shift some SF05 line numbers.

- [Japanese RFC](rfc.md)127–135, [D1 migration](decisions-and-migration.md)13–21, [plan](implementation-plan.md)SF04 row and63: adopted boundaries and future acceptance.
- [Registry](../../../compiler/src/stdlib.rs)275–303/450–591/711–730/1183–1193: resource flags/Passing/borrow_owner/Response/owned transforms.
- [Checked plan](../../../compiler/src/check/checked.rs)580–650/681–715 and [emitter](../../../compiler/src/emit.rs)629–687: canonical operation/constant/field and sealed plan; no separate emitter-name checker.
- [Runtime finalizer](../../../runtime/src/http_server.rs)pinned main3b327–483, [Error](../../../runtime/src/lib.rs)29–77: main lacked BodyKind/CSP; existing MIME/headers/HEAD/empty statuses/ordinary Error form the base. Current C06 slice adds the BodyKind/CSP above.
- [Independent metadata oracle](../../../compiler/tests/resource_contract_characterization.rs)349–405/532–605/686–780, [native app](../../../compiler/tests/security_sf01_native.rs)39–44/109–161, [header tests](../../../runtime/src/http_server/tests.rs)441–484: future connection points, not HTML validation results.

Preserve initial c5acf7b and R01 revision1a0635d as Docs candidates; independent recheck preceded freeze/development adoption at6ef4d21. Core independent review identified no required findings atdfbca0a. Subsequent actual Cargo covered one public core API test; current C06 covered one public factory API test, nine target units, twenty core units and four existing HTTP units; the same binary also passed 260 runtime tests with one existing ignored test. These successes do not close earlier capacity/failed-run records or HTTP ConnectionReset. The following sections record C06 independent review and the limited compiler M01 milestone. SF04 native app/four-OS/browser/TLS/full-feature acceptance remain unverified.

## 2026-10-10 契約freeze / Contract freeze

Independent recheck closed R01 for the revised Docs contract only. Root authorized freezing C01–C08 and development adoption of URL2.5.8 under the [adoption conditions](sf04-url-adoption.en.md). Preserve original candidates/reviews and separate runtime-slice results from later compiler/finalizer integration.

## Compiler milestone M01 (not full-feature acceptance)

The compiler-owned registry now connects six canonical `std.html` resources/twelve operations and `std.http.server.html_response`. C01–C08 signatures, Passing, borrow_owner, flags and seven budgets stay fixed. Existing canonical resource hints/result types, checked facts and the sealed native-call plan carry the behavior. A user's same-name type cannot substitute for the cross-module document argument. Generic copy rejects owned nested HTML through the established payload traversal, distinct from copying shared handles and explicit `copy_fragment`. No new keyword, independent checker or unchecked HTML entry is added.

Actual Cargo first observed the missing-module assertion RED, then passed five targeted tests. The final augmented Cargo run stopped at the disk-preservation guard (exit−15; tests had not run). Root subsequently directly executed the preserved actual unit in a separate stage: HTML6/the identical handwritten whole inventory4/full goldens4/capabilities9 and full unit138 passed. This does not turn the original guard-stop into Cargo success. Temporary unit inclusion of the whole inventory is removed, retaining its original integration registration. The original Cargo guard-stop and the later direct execution of the preserved unit remain separate outcomes.

Evidence covers finite High/saved Low/independently handwritten Low checks and Rust generation, canonical names/ownership/rejection primary lines. SF04 native app execution, nine generated Rust output snapshots, the full native trait compile-fail matrix, original integration Cargo, full workspace/Clippy/four-OS/browser/TLS and completed public docs/equivalent business apps remain pending. One existing small Rust native unit does not prove an SF04 app. M01 independent implementation review (report SHA-256 `4308f7ec4467343c8b9d080b520b5a06cb31449485cd6b4f7a59585f0eddea28`) found no required findings. This is an implementation review; it does not close the listed acceptance work.

## Compiler/native milestones M02–M03 (bounded validation)

Among the historically pending M01 gates, M02 received independent review with no required findings (report SHA-256 `20bd9f3be0d57ee2b1d244c91493978c4aa8d858c8bda954547e4fb7e56ea99e`). It saved the current source's normal compiler Cargo build, ordinary High/saved/independent handwritten Low CLI checks/lowering, nine unedited Rust outputs and three direct-native list/detail main builds/runs. The original Cargo guard-stop stays unchanged; current runtime ef/compiler library and historical artifacts remain preserved.

M03 adds three positive business tests and one checker-negative group in the [persistent harness](../../../compiler/tests/security_sf04.rs). The parent handwrote and froze the386-byte [list/detail body](../../../compiler/tests/fixtures/security-sf04/expected-list-detail.html) before execution; renderer output never supplies that expectation. The renamed-import program constructs its policy/text/document in Nagi, transfers it to the owned factory, and verifies its same-name user type's value. The std.http-only program verifies ordinary text and its local type. Each positive's High/saved/manual Low path, nine in total, builds/runs actual unedited generated root Rust libraries and separate host oracles with the current canonical compiler/runtime, checking complete body/status. Body inspection does not prove wire managed headers/finalization or mapper dispatch.

The persistent negative group directly links the current compiler library and checks six cases across three forms:18 checker refusals after successful parse/load, with reason and primary line in the actual input. Normal `cargo test --locked -p nagic --test security_sf04` and its three nested-native Cargo positives were not launched because capacity/feature-graph cost was not bounded; CI's exact-four registration is not execution success. The existing native_triple helper only gains raw generated-Rust saving before adapter/assertions; timeouts/dependencies/assertions/parallelism stay unchanged. Ordinary release CLI/native Cargo apps, full native traits, mapper/real wire, full workspace/Clippy/four-OS/browser/TLS/MSRV/URL performance/public docs/distribution acceptance remain pending. M03 saved-Low independent recheck (report SHA-256 `abb70e5d54c816b51ee7a538be3b4a542f7ad231b6f1d8593144e61b2546247f`) found no required findings. C01–C08/APIs/flags/budgets/language semantics are unchanged.
