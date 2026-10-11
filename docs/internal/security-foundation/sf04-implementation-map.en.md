# SF04: implementation/RED map (frozen; in development)

[Public contract](sf04-contract.en.md) · [日本語対応表](sf04-implementation-map.md) · [Plan](implementation-plan.en.md)

2026-10-10 UTC. Development after contract freeze at pinned main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`. This map distinguishes future implementation/test plans from the bounded runtime core outcomes summarized below. It does not establish full-feature/native HTTP/browser/CI acceptance. Direct dependency is SF01; do not mix unmerged SF05, reader or editor work into this branch.

## Adoption and implementation order

1. Root and an independent High reviewer review SF04-C01–C08 and freeze adopted signatures/subset/limits/URL/CSP/flags as ordinary RFC concretization. New resource API names, numbers and allowed subsets implement the accepted RFC. New keywords/effects/typecheckers, changes to Task/move/Grant/Tx semantics, weakened security boundaries or unadopted unchecked compatibility APIs are separate public-semantic changes; candidate review alone cannot adopt them. If required, return their reasons/impact for user decision. Do not reopen already adopted D1–D3 breaking migration.
2. If adding a URL dependency, satisfy exact SF04 graph/source/license/MSRV/advisory/target gates and separately record dependency adoption. Consumer compilation does not prove renderer safety.
3. Save small RED oracles against frozen contracts, confirming failure stage/expected diagnostic/source position. Unresolved/parser errors from unregistered stubs are not RED evidence. Stop and repair the harness if typed-API reachability and intended oracle failure cannot be distinguished.
4. Implement canonical metadata→checked facts/sealed plan→opaque runtime renderer→finalizer in small steps. Confirm independently handwritten metadata expectations, High/Low/native agreement and real DOM; separate feature GREEN from environment failures.
5. Finish independent review, appropriate regression and four-OS/native/controlled TLS/browser oracles before acceptance. Units/mocks cannot replace unexecuted browsers. Sync public docs/AI examples/translations to final adopted APIs.

Step1 froze at6ef4d21; step2 records development adoption with remaining conditions. Preserve undefined-API/module behavior evidence and record subsequent actual-Cargo public core API1. C06 proceeds from four existing-API compiled CSP REDs to runtime finalizer implementation, recording current public factory API1/unit9 and same-binary core20/existing HTTP4/full runtime260passed/1existing ignored. The following sections record independent C06 review and the limited compiler M01 milestone. Native SF04 app/four-OS/browser acceptance remain later work.

## Connection points and finite oracles

| ID/area | Connection point | Positive oracle | Negative/boundary oracle and rejection stage |
|---|---|---|---|
| SF04-C01 canonical resources | compiler/src/stdlib.rs Resource/StandardModule/ResourceContract and independent resource_contract_characterization expectations | Complete six-resource inventory/flags, all operation signatures/Passing/borrow_owner, Tag constants/canonical native paths checked against handwritten expectations | Reject same-name user-type spoofing/private fields/wrong resources/contexts/Copy/Serde/Debug derivation at checker source position. Accepted storage/shared/cross-task cases must build with native traits |
| SF04-C01 sealed lowering | Existing compiler/src/check.rs, check/checked.rs, emit.rs plan | Thirteen operations through High check→generated-Low reparse/check→Rust build and saved/manual Low with identical ownership | Existing checker rejects reused moved attributes/fragments/documents and owner drops across borrows, including alias/Result/loop combinations. Nagi-accepted Rust move/lifetime rejection is P1, not GREEN |
| SF04-C02 static subset | runtime/src/html.rs and closed HtmlTag metadata/constants | title/href order, void BR, UL/LI, DIV/phrasing, fixed document wrapper in exact bytes/DOM tree | No dynamic/unsupported names at public API/type boundary; nested A/list text/orphan LI/flow inside phrasing/nonempty BR return Error::Invalid. Arbitrary metadata registration cannot bypass it |
| SF04-C03 contexts | Private text/quoted-attribute/document-title encoders | Small Unicode/five-character/TAB/LF/CR/literal-entity fixtures and identical logical DOM values after join/copy | NUL/forbidden controls are Invalid; str/bytes casts to fragment/NavigationUrl are compiler/native-consumer rejections. DOM/bytes detect reescaping/reinterpretation |
| SF04-C04 navigation | Private bounded URL adapter and HtmlPolicy/NavigationUrl | Canonical HTTPS base IDNA/IPv4/IPv6/default port, root-relative query/fragment, final single-leading-slash href/re-resolution against fixed base/equality to intended canonical URL, HTML encoding of href | Post-normalization network-relative forms and reinterpretation changing origin/path/query/fragment; absolute/network-relative/backslash/control/space, userinfo/root-dot/non-root base and differing profiles are Invalid. Parse-only negative cases, no DNS/third-party connection |
| SF04-C05 budgets | Private summaries/checked arithmetic/preflight buffer sizing | Lower limits for small inputs at limit−1/exact/+1; independently calculated wrapper/title/URL/copy/node/depth costs | Reject quota resets/overflow/profile mixing/double-count omission after copied-fragment join/zero-negative-over-limit values as Invalid. No huge inputs/exhaustion. Record finite allocation-measurement conditions |
| SF04-C06 body classification | runtime/src/http_server.rs private BodyKind/representation/finalizer and stdlib HttpHtmlResponse | Small owned HTML response MIME/CSP/nosniff; unchanged text/bytes/JSON MIME; mapper/early errors/HEAD/204/205/304 | Bytes/headers/manual Rust conversions cannot change to active MIME. Reject all reserved fields and HTML Content-Encoding; compare preserved nonHTML behavior |
| SF04-C06 standard Rust | Runtime public exports/private fields/traits | Same &str/owned-resource validation and Send/Sync compared with contract | Compile-fail from_raw/unchecked/Deserialize/Display/Clone/private fields/BodyKind setters/HTML MIME override. Custom trusted Rust is not the standard API |
| SF04-C08 raw migration | Existing Operation::Html tombstone/checker diagnostics/error_routes and separate40877b3 coverage | Real migrated typed list/detail app preserves business content/status through High/Low/native/browser | No old raw entry via direct/import alias/function value/saved/manual Low/native_low/@replace. Parser failure/ICE/unrelated failures do not establish migration success |

No new keyword/effect/independent solver. Reference views and borrow_owner=None owned results follow the existing registry path; do not recreate move logic using emitter name lists. Native/@replace checks also require checked standard Rust factory types. Arbitrary custom Rust authority is not a sandbox guarantee.

## Concrete small app, transport and browser oracles

After signature adoption, build one owned list/detail fixture: business title, Unicode item description, optional quoted title, UL/LI, A to `/items/7?view=details`. Preserve existing policy-required public/protected route, Grant and business Err. No source fixture is authored now; this is the shared semantic plan.

| Observation | Recorded evidence | Boundary |
|---|---|---|
| Handwritten metadata | Complete resource/operation inventory/signatures/flags/Passing/borrow_owner | Do not derive expectations from the registry itself |
| High/saved/manual Low | Parse/check, migration/ownership source positions, generated Rust build, equivalent app execution | Check success is not Rust/native success; undefined new APIs are not RED |
| Runtime units | Encoded bytes/summaries/profiles/low-budget boundaries/managed headers | Not evidence of browser interpretation |
| Owned native loopback | Wire status/MIME/CSP/nosniff/HEAD/empty status, handler/mapper Error/panic/deadline/cancellation/capacity release | Existing SF01 results do not prove the new typed path; retain non-yielding/post-start limitations |
| Controlled TLS/browsers | Chromium/Firefox/WebKit versions/OS, DOM structure/text/quoted attributes, final href destination equal to intended canonical origin/path/query/fragment, rejection of post-normalization network-relative forms, actual headers | Mocks/string searches do not replace DOM/TLS. Inert structural fixtures, no script execution/attack payloads |
| Four native targets | New dependency/build/runtime on Linux/Windows/macOS ARM/macOS Intel | Past four-OS CI or harness registration is not new-feature completion |

Observe malformed text/attribute/URL rejection using small owned fixtures, with no third-party scan/external request/huge input/PoC/exhaustion. Check CSP's fixed directives as headers; blocked script/style execution is not an encoder proof.

## Later public-document synchronization

On adoption/implementation, update docs/http-server.md and its English counterpart, resource/operation references, AI HTML migration examples and equivalent list/detail apps. Preserve old operation-import and Response body/header borrow/ownership semantics; state unsupported forms/style/script. Do not describe existing SF01 migration docs as completed typed rendering.

The development record keeps R01 Docs closure, the prior 6ef4d21 freeze/development adoption, compiled core module RED/GREEN, and the actual-Cargo capacity stop as distinct outcomes. Metadata/finalizer/High/saved/manual Low/native HTTP/browser/four-OS acceptance remains incomplete; contract adoption and each layer's results are distinct.

## Compiler M01 scope and next checks

Added six resources/twelve HTML operations/one HTTP factory and twelve Tag constants to compiler-owned metadata. Canonical hints/result types belong to the existing standard-call checker; Passing/move/view to existing checked facts; native paths to the sealed plan. The established purpose-specific payload walk rejects non-Clone HTML in owned wrappers while excluding shared handles. The independent whole inventory extends to44 resources/95 operations/12 constant sets. Only the new factory DefId/binding and Rust reexport were added to two HTTP full goldens; existing bodies and metadata order stay intact.

Initial metadata assertion RED, five targeted actual Cargo passes, the final Cargo guard-stop, and later direct runs of 23 actual-binary checks and all 138 units are separate outcomes. High/saved/handwritten Low cover13 operations, renamed imports, a same-name user type and HTTP-only imports. High/saved negatives fix check stage, reason and primary line in the actual input; manual Low confirms two negatives for same-name document and owned nested Clone. Temporary unit inclusion of the integration table is removed from final source.

M01 independent review (report SHA-256 `4308f7ec4467343c8b9d080b520b5a06cb31449485cd6b4f7a59585f0eddea28`) found no required findings. Remaining work includes unedited snapshots of nine emitted Rust outputs; bounded original integration/CLI/Cargo/native SF04 app checks; the complete native opaque trait/visibility matrix; equivalent business-app migration; public docs/AI examples; four-OS/TLS/browser/distribution gates; and full-feature acceptance. The uncompiled snapshot-output hook candidate is excluded from this source milestone and is not counted as actual snapshots.
