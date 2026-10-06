# Task S2 native/service evidence

[結果と限界](../../../docs/internal/task-handles-s2-results.md)・[provenance](provenance.json)。production基点はS1 merge `aee1987a7ede5eeebb5e253afd65cb5ede327dde`、compiler/runtime/manifest全64ファイルは変更していない。測定済みS1のruntimeコストを保持し、S2サービスの速度優位・ゼロallocationを主張しない。

snapshotは新oracle、実High/手書きLow例、library verifier、CI登録。logsは失敗を含む原ログと独立review、三構文生成Rust/実native stdout。native binary、Cargo cache、website生成物は含めない。rootと独立reviewの7ケース×3構文は別に数え、filterされた既存testsを成功へ足さない。初回adapter/Low記法失敗は最終成功ではない。

保存時点のchecker110/110、fmt/clippy、公開両例三構文、独立reviewは成功。workspaceは95 result block・935成功・failed0・費用用ignored1、全library15検証が成功。4 OSとmerge/releaseは未完了。追記時は実head/tree、source hash、ログSHA-256と保証範囲を読戻し、このsnapshotを後続source検証へ流用しない。
