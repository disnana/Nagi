# 登録資源の契約: 先行テストの結果

2026-10-05。Phase 2の成功head `27c8bf4`を基点にしたPhase 3。採用設計は[ADR 008](adr/008-resource-contracts.md)。このcommitはtest-onlyで、ResourceContractの本実装はまだ変更していない。CI成功後にだけ集約へ進む。

## 独立した期待と生成例

- [`resource_contract_characterization.rs`](../../compiler/tests/resource_contract_characterization.rs): 22resource、47operation、32 accessorと全constants。署名、generic/value arity、Passing、borrow_owner、集合完全性、ResourceInfoの公開fieldと戻り値形を独立した手書き期待で確認する。
- [`capabilities`の用途別test](../../compiler/src/capabilities.rs): inline/shared payload、Actorの間接protocol、callback signature、nominal phantomを区別する。native Debug、Serde、Charge、owned field storageを同じ条件にまとめない。
- [`resource_contract_goldens.rs`](../../compiler/src/tests/resource_contract_goldens.rs): HTTP検査、Borrow/Mapper、Actor data、data deriveの4例。実resolverへ固定logical ModuleIdを渡し、High check→Low→独立finalizeと直接finalize→Rustを通す。metadata・symbol・alias・deriveを削らず、8ファイル計99,254 bytesを比較する。
- [`http_codegen.rs`](../../compiler/tests/http_codegen.rs): 既存実fileのHigh/独立Low比較を維持。追加Cargo例は非Copyの文字列DTOをJSONへ借用し、App/routeへnamed mapperを登録する。mapper本体やHTTP dispatchの実行を新しい保証にはしない。

固定logical IDのgoldenは実fileのprovenanceを保証するものではない。元位置・実HTTP・Actor・共有field・owned Copyのnative検査は既存harnessを維持する。旧HTTP/Actor sourceは抽出前の1,241/245 bytesと一致し、既存assertは削除していない。

## 先行検査中の失敗

4goldenは最初に空の期待値との差で失敗し、実生成結果を採取して固定した。採取用の一時コードは撤去し、通常testから期待fileを更新する仕組みは残していない。期待8ファイルは採取bytes・SHA-256と一致する。これは新機能のfailing testではなく、現行生成のcharacterizationである。

手書き定数oracleの初回ではCallKind.NOT_READYの転記漏れ1件を検出した。既存登録を再確認し、oracleだけを訂正した。production、既存test期待、受理・拒否は変更していない。最初のclippyのtuple complexityはtest用の型aliasで解消し、allowを追加していない。

CI配線の回帰は、4 OSの明示一覧に新inventoryと既存shared-field testがない状態で失敗した。一覧へ追加後、選択を確認するPython testが成功した。版変更がないときの非公開規則は維持する。

## ローカル検証

- 対象125成功: library 113、inventory 4、HTTP codegen 4、Actor codegen 2、owned Copy 2。新規は13件。関連nativeのHigh/保存Low計6 runも成功した。
- `cargo test --locked`: 91 suite・820成功、失敗・ignoreなし。
- fmt/clippy all-targets: 成功。
- CI判定のPython 52、release scriptのPython 90: 成功。release testsは模擬入力であり、実際の公開ではない。
- conformance登録検査: corpus 38、linked harness 16。登録確認を実行成功には数えず、対応Cargo testの実行は上の結果とCIで確認する。
- website: 90ページを生成し、links/anchors/assetsを確認した。

Solが登録期待と生成testを実装し、別のSolがfreeze差分を独立レビューした。rootも旧入力・production不変・hash一致・全suiteを確認した。レビュー自体を言語の完全性の証明にはしない。生ログとhash証跡は作業環境の`/workspace/test-tools/compiler-rust-boundary-plan/phase3-test-logs/`と`phase3-test-only-full.log`に保存した。

## CI・残課題

先行test-onlyの4 OS CIはまだ未確認。成功前にResourceContractの集約を実装しない。#79未マージの間はstacked PRで依存を明記し、最終反映先はmainとする。こちらではmerge・版更新・releaseを行わない。

[Copy深さの差](copy-boundary-investigation.md)は別のP2として記録した。今回のinventory集約で受理・deriveを変えたり、差を正常goldenへ固定したりしない。Pool/Tx/lifecycle・新しい保証は未実装。既存Rustへのtrait/Send/Sync/link/依存環境の委譲も維持する。
