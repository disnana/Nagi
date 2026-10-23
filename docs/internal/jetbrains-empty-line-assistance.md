# Empty-line queries and source facts

An unindented final empty line in a valid High or Low file must not remove that
file's current symbol index. Published compiler 0.2.1 replaces the cursor's empty
expression with `True`; at top level that recovery fails to parse, and its early
return omits the index even though the original input passed frontend checking.
The plugin's shared caret/baseline request then publishes empty semantic colors,
documentation, and navigation. The completed `Point` class with named `f64`
constructor arguments is valid; this failure does not require invalid source.

## Compiler boundary

`frontend_checked` means the original source/native graph loaded and parsed;
`frontend_accepted` additionally reports the normal frontend check/finalization
result. Neither flag claims a Rust build. These verdicts and diagnostics always
describe the original request, not its query recovery.

When recovery fails to load but the original source loaded, use that original
AST with no checker query to build the current index. Discard the synthetic
cursor and set `recovered:false`: no recovered program supplies these facts.
Completions remain empty. Original type errors remain rejections; original parse
errors remain diagnostics without fabricated facts. Successful member and
indented-expression recovery retain their existing checker/ownership rules.

## Published-compiler compatibility

Keep the plugin's shared baseline/cursor request. Decode whether `symbols` is
present separately from its lists: an explicit empty index is authoritative.
Only a query response with `frontend_checked:true`, no index, no import context,
no candidates, and semantic status `none` requests one queryless baseline.
The request uses the same immutable overlays, plan, worker, cancellation token,
and remaining operation timeout. Fixed compilers and successful queries incur
no second analysis. Original parse failures do not retry.

Before merging, revalidate snapshot generation, cancellation, executable and disk
identities. Merge only source files, definitions, locals, roles and references;
preserve the query response's verdict, diagnostics, completion context and
candidates. Publish once after the existing dependency/final snapshot checks.
Never reuse facts from a previous buffer or read saved source instead of an
unsaved overlay. A second omitted index ends the retry. Stable verdict/diagnostic
disagreement is a protocol failure and uses the existing failed-request cache;
actual snapshot changes follow the existing stale-result retry path.

Both requests share one absolute monotonic deadline held by their cancellation
token. The session waits for the response using the remaining nanoseconds; it
does not round a fractional remainder up to another second. Expiry stops and
reaps the session through its existing transport cleanup, then reports an I/O
failure (`FAILED`), distinct from editing cancellation or stale-result retry.
The service also checks the budget after each response, after merging facts and
immediately before making the cache current. Fractional time that remains may
still produce a valid response. The one-shot transport uses the same budget for
its existing waits and output collection; its existing synchronous stdin write
and process cleanup are unchanged and are not claimed to be deadline bounded.

## Focused coverage

`compiler/tests/editor_assistance.rs` covers High/Low final empty lines, original
type and parse errors, saved/overlay requests, EOF and imported project graphs;
existing cases cover member/indented recovery, UTF-16 and persistent processes.
JetBrains protocol/service tests distinguish missing/empty indices, preserve
query verdicts, bound retry counts, and cancel obsolete fallback requests.
The registered editor integration test types malformed then valid `Point` code,
stops on the final empty line, and checks actual colors, documentation,
navigation and subsequent field completion. Run that case with both the matching
compiler and published 0.2.1 via `NAGI_TEST_COMPILER`; a fixed compiler alone cannot
verify the plugin's compatibility retry. Headless checks do not replace GUI
acceptance on the supported IDE versions.
