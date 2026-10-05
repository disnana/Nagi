# バックエンド境界の次フェーズ監査

基点は未マージのPR #74 (`6d5b9ee`)。このフェーズで#74やmainを更新せず、派生ブランチで作業する。#74の全CI成功は前フェーズの結果であり、新しい実装の検証結果ではない。

## 維持するもの

- checkerのoperand facts、借用元とcleanup anchorを分けたstorage plan、Scopeの同coroutine生成、envのlazy fallbackを維持する。
- High→Low text→再check→Rust、保存Low、手書きLow、`@replace`を残す。生成したRustを後から修正して不具合を隠さない。
- 31回帰source、実Drop/Scope/HTTP/SQL harness、生成・縮小・失敗保存・週次探索を土台にする。期待値を現実装に合わせて弱めない。

## 新方針との衝突

| 論点 | 現状・衝突 | 方針 |
|---|---|---|
| Rust資産 | 同一Cargoビルドのtyped externがある。任意Rust型・traitの自動公開はない | この接続を育てる。C ABI、独自HTTP/DB再実装、任意crate自動変換へ広げない |
| check | 型と所有権は一つのwalker。最終Low checkのfactsは生成へ渡せる | 独立したdomain validationを型検査後へ置く。全面HIR移行はしない |
| 定数算術 | compound/alias zeroとMIN/-1を受理しrustcで拒否。overflowは既存profile依存 | 型付き共通validation passで静的失敗を検出する。release値をdebugの意味としてfoldしない |
| 認可証明 | 通常classは構築・JSON復元できる | DTOの型として使い、認可証明には封印したopaque型を使う |
| HTTP | 標準Hyper実装と旧Axum APIが共存 | 既存APIを削除・置換しない。新サンプルはAxum/Tokio/Serdeを直接利用する |
| source map | 元ファイル・定義/文の行と関連noteを戻せる。式の正確な列は保持しない | 行精度を明示する。推測で列やRust修正提案をNagiへ移さない。生成Rust詳細を明示オプションへ分ける |
| auth境界 | route/SQL名から権限を推測する仕様はない | 初回は明示した保護adapterが消費型の許可証を要求する。全route/全情報流の証明とはしない |

## 採否と順序

決定は[ADR](adr/001-backend-boundaries.md)、[定数評価](adr/002-constant-validation.md)、[診断](adr/003-diagnostic-boundary.md)に記す。まず契約・negative testを置き、その後に小さい実装、High/Low/native、実socket、比較計測を通す。

新しい予約語、Low廃止、runtime全面交換、一般effect/taint、独自borrow runtime、全面zero-copyは今回採らない。共通CheckedProgram/HIR、一般opaque Rust型、request単位の権限flow、SQL policyは追加設計の対象。名前を付けただけで実装済みにしない。

## レビュー単位

#74は維持し、次フェーズは別PRとして分離する案を推奨する。定数評価・診断・Auth/interopが大きくなればさらに順番に分ける。依存PRのbase/headとmainへの最終反映先を明記し、feature branchへの反映をmain反映と報告しない。mainへのマージは行わない。

実装後の検証と未解決点は[結果報告](boundary-foundation-results.md)に記す。監査時の課題と、修正後の状態を混同しない。
