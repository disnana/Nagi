# Nagi 0.2.2 completion inventory

Updated 2026-10-25 against main base `22183d7`. Nagi 0.2.1 and JetBrains 0.1.6 are published; 0.2.2 is development work. This index separates present implementation, stale status text, a candidate still awaiting implementation, and future design. It does not replace historical evidence or publish a release guarantee.

## Japanese

| 区分 | 現在の状態・根拠 | 残す理由 |
|---|---|---|
| 実装済み／古い状態記述 | SQLite Pool/Txは0.2.0で公開済み。現行実装は[sqlite module](../../runtime/src/sqlite/mod.rs)、Txは[session](../../runtime/src/sqlite/session.rs)。[未決事項](open-questions.md)に残るPR #99 draft／CI pendingは作成時点の履歴。#150はmerge済みの[commit](https://github.com/disnana/Nagi/commit/9a3a975999b11fd95b49cca077ef989258bc9fe4)。 | 当時の判断・検証経緯を保存しつつ、現行の未実装と誤認させない。 |
| 実装候補（未実装） | **cold initial Cargo.lock seed boundary**。現行[世代作成](../../compiler/src/generation.rs)は既存lockを継承し、ない場合はCargoに解決を委ねる。[世代回帰](../../compiler/tests/build_generations.rs)は既存lockの保持・失敗時挙動や不正lock拒否を検査する。初回seedの由来／更新境界を独立に固定する専用候補はまだ実装されていない。 | Cargoに解決を委ねる既存契約や利用者のlockを不用意に変えないよう、対象と保証を決めた後に小さく扱う。候補であり、0.2.2の約束ではない。 |
| 将来設計 | HTTP応答の圧縮展開は現行sourceに未実装。HTTPは[identity-only契約](../http-client.md#接続動作と現在の検証範囲)を保ち、[回帰](../../runtime/src/http_client/operation_tests.rs)で非identity拒否を確認する。生成Rust診断の列位置も[source map](../../compiler/src/source.rs)が行単位で、provenanceの設計が必要。一般package discovery、任意Rust resource登録、PostgreSQL標準APIも将来項目。 | codec・サイズ／時間上限・取消後join等が未決。列位置はcompiler pipeline全体の出所対応を要する。package/resource/DB追加は互換性・ownership/API契約を別途決める必要がある。 |

## English

| Class | Current state and evidence | Why retain the note |
|---|---|---|
| Implemented / stale status text | SQLite Pool/Tx shipped in 0.2.0. Current code is in the [sqlite module](../../runtime/src/sqlite/mod.rs) and [session](../../runtime/src/sqlite/session.rs). PR #99 draft/CI-pending wording in [open questions](open-questions.md) is historical. #150 is merged at [commit](https://github.com/disnana/Nagi/commit/9a3a975999b11fd95b49cca077ef989258bc9fe4). | Preserve the decisions and evidence from that time without presenting them as current blockers. |
| Implementation candidate (not implemented) | **Cold initial Cargo.lock seed boundary.** Current [generation creation](../../compiler/src/generation.rs) inherits an existing lockfile and leaves resolution to Cargo when none exists. [Generation regressions](../../compiler/tests/build_generations.rs) cover preserving an existing lock on failure and rejecting malformed locks. A dedicated candidate to pin the origin/update boundary for the first seed has not been implemented. | Define the target and guarantee before changing Cargo's existing resolution behavior or a caller-owned lock. This is a candidate, not a 0.2.2 commitment. |
| Future design | HTTP response decompression is absent from current source. HTTP keeps its [identity-only contract](../en/http-client.md#connection-behavior-and-validation-status), with [regressions](../../runtime/src/http_client/operation_tests.rs) for rejecting non-identity encoding. Generated-Rust diagnostic columns also need provenance design; the [source map](../../compiler/src/source.rs) is line-based. General package discovery, arbitrary Rust resource registration, and a standard PostgreSQL API remain future items. | Codec and size/time/cancellation/join rules remain undecided. Columns require source attribution across the compiler pipeline. Package, resource, and database additions need separate compatibility, ownership, and API decisions. |

For release and PR status, use the dated current-state notes in [open questions](open-questions.md) and [the 0.2.2 plan](nagi-0.2.2-plan.md). Individual PR discussions remain the source for their own status; this inventory does not restate their contents.
