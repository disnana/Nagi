# SF04 URL dependency: development adoption and build inputs

[日本語](sf04-url-adoption.md) · [Contract](sf04-contract.en.md)

2026-10-10 UTC. Development adoption for implementation, not complete SF04 acceptance/distribution/safety proof. Root authorized runtime `url = { version = "=2.5.8", default-features = true }` and the reviewed exact lock (SHA-256 `dc308b07a5ebfe3920ff552e8f44c7b2dbc8c54608cfe3aafb54de6f7f996892`). C01–C08/final-href rules remain unchanged.

## Evidence and choice

Independent `SF04-URL-DEPENDENCY-R01` (SHA-256 `38528fb4a847e46b06ab875be656f148fd492563c72e16054d624f4810bc7961`) found no required factual correction/demonstrated version or graph blocker. Exactly25 additions,zero removals/replacements. URL default/std→IDNA1.1.0 alloc/std/compiled_data→ICU2.3 compiled_data; existing workspace union adds only syn fold. Pinned research/review checked archive/checksum/manifest/license texts. Four-target metadata/tree selection is not four-OS compilation.

Maximum declared runtime-graph MSRV is1.88 (baseline1.85), not inferred from URL1.63/IDNA1.57 alone. Actual toolchain is1.99 only;1.88 is unmeasured. Fast's benign offline URL consumer exited0 in4.008s and checked four ordinary example.invalid URLs' origin/path/query/fragment/empty-versus-absent/IDNA only. It proves neither renderer/R01 final-href policy/browser/HTTP/native app/four-OS nor minimum-toolchain support.

Fixed official RustSecDB `7eebec69c352c7191b1f13eb95dd510eeca5d1de`, date2026-10-09, archive SHA-256 `c71855c79d75ef2133c703d7034037b4674975690bed0594857ddbe629faf469`, yields zero affected candidate versions in the version-range metadata comparison. Distinguish patched-range/informational results and unverified reachability/aliases/Git/path/features/targets/formal-scanner policy; this is not cargo-audit completion or safety approval.

Pinned and saved-current URL/IDNA SECURITY.md retain a2.2.x table. It establishes neither current support nor unsupported status for2.5.8/1.1. Adopt with unknown maintenance scope. Nagi maintainers own updates: recheck exact source/graph/licenses/current DB when updating locks, track upstream support and update/replace dependencies when necessary. Release recency/yank=false do not promise support.

## ICU build-input boundary

The two added data packages' build.rs/lib.rs use external includes when `ICU4X_DATA_DIR` exists, including an empty value. Archive/lock pin alone cannot identify that source. Standard SF04 validation child processes leave this variable unset and record host/child presence only, never its value/secrets. Use reviewed bundled compiled data; custom data requires separate source/hash/license review.

Connect this condition to SF04 target-CI input/environment checks and actual build logs recording presence/bundled source identity. Apply/verify it to generated-app builds using the same runtime features/lock; an unrecorded ambient override is not standard validation. Do not globally sanitize the compiler environment/change arbitrary host Rust environments or claim CI integration exists. Apply it to the bounded slice runner now; CI/generated-app integration remains acceptance work.

## Remaining acceptance/distribution conditions

- Independently validate actual consuming-workspace feature union/URL adapter and typed renderer with small RED/GREEN.
- Measure intended minimum toolchain, Linux/Windows/macOS ARM/Intel build/runtime, real browser DOM/TLS/origin interpretation.
- Before distribution, bundle notices for new25 packages: full Unicode V3 including IBM copyright/attribution, MIT and selected dual-license terms. Another worker owns that notice draft; this engineer does not edit it. Existing-project notice review remains separate.
- Retain unresolved current-maintenance evidence/advisory policy/date/lock/source reachability/formal-scanner boundaries during adoption updates.
- Verify and record absent ICU override in CI/generated-app builds, or separately review custom data sources/hashes/licenses.

Choose the fixed resolver-feasible dependency for development to avoid a custom URL grammar and implement strict raw→canonical final href→re-resolution→encoding with one parser. No general safety,indefinite support,formal audit or all-target-success claim follows.

Select MIT for dual-licensed additions and retain each shipped MIT/Unicode V3/IBM copyright/permission/attribution/COPYRIGHT text. Another worker/root preserves the notice bundle and connects the distribution gate.
