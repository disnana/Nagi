# 条件付きTask消費のREDとchecker修正

ADR012の全T正常出口義務に対する実装修正。短絡and/orのRHS、lazy env fallbackだけでawait/discardすると、skip経路の未受取を旧checkerが見逃した。生成Rustの条件付き評価を維持し、checkerの既存branch joinへskip状態を戻す。新しいAPI/runtime/依存/意味論は追加しない。結果正本は[内部記録](../../../docs/internal/task-conditional-consumption-fix.md)。

- `independent-before-fix/`: Sol Maxが未修正candidateで発見したP2 blocker、2群×High/保存Low/手書きLowのparse/check/seal/build/native成功とreceipt marker欠落、present/missing同じexe control。手書きreview harnessのpath解決失敗も訂正履歴と原ログで分離する。
- `engineering/`: 先行134契約の114一致/20誤accepted、修正後148/148、入力登録148/74対、元行/診断oracle、三構文native7経路、関連view/move/constant/fmt/clippyの原ログ。最初のsocket PermissionDeniedと中止されたnetwork再実行は成功へ数えない。native artifactはsource/Rust/生stdout/stderrのみ。
- `snapshot/` と `provenance.json`: 変更source44ファイルを実SHA-256照合して保存、期待fixtureと原artifact hashを固定。この記録自身のcommit SHAは自己参照させない。
- `view-control/`: 条件付きでview-containerを渡した後はcontainerを再使用せずownerをmoveする対照例。旧CLIで実build/nativeが成功したので、新しいloan factの変更や別blockerへ広げない。

全workspace/HTTPの再確認、独立した修正後review、最終headの4 OSとmain反映は、別の読戻し追記まで未完了。0件filter・skip・費用ignored・infra失敗は契約GREENに加えない。binary/cacheは保存していない。
