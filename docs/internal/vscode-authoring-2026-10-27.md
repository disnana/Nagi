# VS Code authoring: published version, main and pending syntax

Baseline: main `0d1884cb5a24235a440811a9f5d16be1bf5a664a` (2026-10-27 inspection).
The PR base was refreshed to `822b38d01aa9e90f3290e4463880ae1fac2d4deb` after
the tutorial merge; compiler/runtime/VS Code sources are unchanged between them.
Marketplace's public query reports `Disnana.nagi-lang` 0.1.13. The GitHub
`vscode-v0.1.13` VSIX has SHA-256
`86f64f16c913f3af60a0a877d65b0c83a295c45339bb505d1ff2485bb979043f`.
Its bytes differ from main even though main still uses version 0.1.13. Neither
the version string nor a local package establishes that main changes are public.

| Area | Published 0.1.13 / compiler 0.2.1 | Main baseline and pending work | This change |
| --- | --- | --- | --- |
| `elif` | Not supported by published compiler or keyword/snippet assistance | #180 implements High parsing, VS Code keywords/color/indentation/snippets; unmerged | No duplicate implementation |
| Named ordinary function calls | Existing insertion is positional | #181 adds compiler support for all arguments in declaration order; unmerged, no VS Code files | Keep positional function insertion; do not advertise unmerged syntax |
| Class/record named fields | Compiler 0.2.1 accepts reordered fields; hints incorrectly count commas and label completion is absent | Same editor gap remains | Complete remaining field labels and select the declared field by its actual label |
| `print` and builtins | TextMate call color, completion, hover and signatures exist | #173 synchronizes builtin vocabulary and removes retired spellings; #171 adds `read_line` overload descriptions | Preserve existing behavior and shadowing guards |
| Type input | Existing type-position completion and class insertion are present | #173 synchronizes 24 supported type spellings, including `Map`; do not offer internal `Future`/`Range` as new recommended types | Preserve type-only completion and full class templates |
| Snippets | Existing positional function and named class templates | Main has updated HTTP examples; #180 separately owns `elif` snippets | No snippet or compiler-version change |
| Compiler contract | `symbols` provides class declarations/fields; compiler 0.2.1 rejects incomplete label syntax | Main `read_line(prompt)` help requires the newer compiler implementation; published 0.2.1 is not silently upgraded | Continue using existing `symbols` and overlays; no `assist` migration or new CLI requirement |
| Go/HTTP pending work | Independent compiler/runtime concerns | #182/#184/#185/#186 are stacked Go changes; #183 concerns HTTP resilience | No dependency on or edits to those branches |

TextMate call color is lexical, not proof that a token resolves to a builtin.
Project hover/navigation use compiler facts. This PR does not add a semantic
token provider or claim custom type/value coloring beyond the existing grammar.
Field-label completion displays the declaration's type; it does not approve an
argument expression, assignment, move or ownership operation.

## Finite behavior

- For a compiler-described class/record constructor, offer only unsupplied field
  labels at direct argument-name positions. Check both sides of the caret;
  preserve an existing `=` and its value, and otherwise insert `name=${1:name}`.
  Carry the existing-equals fact from the same comment-masked argument context
  into insertion, so a comment/newline between label and `=` is preserved too.
- Use the actual field name for the active signature parameter, including
  reordered fields, multiline calls and qualified current namespaces. An unknown
  explicit field label supplies no misleading parameter hint.
- Do not replace value expressions, nested call arguments, comments, strings,
  ordinary function arguments, member access or type-only completion.
- Keep the existing saved-declaration fallback explicit in detail/documentation.
  It is declaration assistance, unlike receiver-value field completion, which
  remains disabled for saved snapshots. Suppress new saved label candidates
  when imports or lexical shadowing make the callee ambiguous; qualified label
  assistance requires a current compiler namespace.
- Keep existing cancellation/version/trust guards and process counts. No new
  analysis request, source recovery, compiler execution path or dependency is
  added. Compiler errors on incomplete input remain errors.
- Bound call exploration to 16,384 UTF-16 code units, 128 delimiter levels or
  unmatched opening-parenthesis probes, and 1,024 units for a callee prefix.
  Forward field-label scanning uses the same scan/depth limits. Stop without
  constructor candidates/signature facts when a limit is reached; never resolve
  a truncated qualifier as a new global name. Reuse completion's masked source,
  and inspect only the bounded prefix at each opening parenthesis. The existing
  full-source string/comment masking remains linear in buffer size.

## Verification boundaries

Focused cases cover reordered labels, supplied fields before/after the caret,
existing equals signs, nested arguments, values/comparisons, shadowing, saved
warnings, qualified classes, UTF-16/CRLF and High/Low registered providers.
A finite child-process case exercises 100,000 unmatched delimiters and excessive
call spacing, and checks that a bounded prefix cannot drop a qualifier. Legal
High/Low comment-separated labels are checked by the real compiler and provider.
Real compiler cases exercise both main and published 0.2.1 `symbols`, preserve
the original parse rejection and leave the saved file unchanged. The existing
full VS Code suite and VSIX packaging remain required. Headless provider tests
do not establish real GUI theme rendering or user acceptance; the PR remains
Draft and no release, Marketplace upload, tag or merge is part of this work.
