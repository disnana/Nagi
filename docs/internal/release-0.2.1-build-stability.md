# 0.2.1: ビルド高速化と安定性の作業記録

対象base: `e2d728006dbfe49eb8a382f88ce777f29284ed4c`。公開版はまだ0.2.0。
版更新・リリースは検証済み差分と分ける。#145のJetBrains GUI受入は別作業。

## 既存実装と今回の順序

既存の[全体計画](smart-build-plan.md)、[生成本体CAS](generated-body-reuse.md)、
[関数依存単位](function-unit-reuse.md)を土台にする。生成Lowの再parse/check、
sealed emission plan、世代のOS lock・旧exe保護、Cargo.lock継承、Cargoによる
native fingerprint再判定は既にある。これらを新規実装や高速化実績として数えない。

`graph/{modules,calls,types}.rs`の表示用graphはunknown callback等を完全には追跡しない。
現在のunitsは検査済みplanとASTを照合した閉じたprimitive同期関数だけをSCCでまとめ、
最大8 Cargo libraryとfacadeへ分離する。公開APIと本体のhashは観測できるが、
型・所有権検査queryやテスト結果の再利用は未実装。解析上の関数粒度とCargoの
再利用粒度は異なり、callee crateの変更はCargoの逆依存再ビルドを起こす。

1. **P0・重複処理**: physical moduleをparseした直後の同一ソースの再lexを除く。
   ASTのspanに使ったtoken配列自体をloaderへ渡す。独立した生成Low検査は残す。
   import traversal中の親token保持が長くなるため、メモリ低減は仮定せず測定する。
2. **P0・repository CI**: regression jobの後続testが使わないrelease compiler buildを除く。
   package jobのrelease buildと配布検証、全regression stepは残す。
   frontendのnative fixtureはinput/outを個別に保ちCargo targetのみ共有する。
   これはNagi利用アプリの速度とは別の指標として報告する。
3. **P1・次段階**: primitive署名・既存のbody allowlist・sealed planで閉じた
   async leafのunits対応を検討する。Await/Task/borrow/resourceを一般に許可する変更とは分ける。
   効果とFutureのpoll/取消/Drop、High/保存Low/native一致を確認してから採用する。
4. **P2以降**: 型を分離するにはRustのnominal identityを保つ共通types crateが必要。
   現在のfacadeに依存するunitや複製型は作らない。query cache、外部共有native cache、
   履歴に基づく自動分割は別acceptanceとし、未検証入力をNagi hashだけで再利用しない。

## 計測と受入

変更前debug CLIをローカルに固定した。SHA-256:
`713a1a1bf0352630c9c1e74de9a6570a0992e802753f8712b45e3faebf1a9635`。
`scripts/benchmark_frontend_checks.py`は同じphysical path・交互の実行順でHighと保存Lowの
完全検査を繰り返し、wall/CPU/peak RSSのraw sample、中央値・範囲を保存する。
診断と生成Lowが一致しなければ失敗する。これはRustビルド速度の測定ではない。
release compiler同士の再計測とchecksum付きnative loopも行った。
[raw sampleと条件](../../benchmarks/results/release-0.2.1-build-stability-2026-10-22/README.md)に
debug/release、cold/warm、CIとアプリの指標を分けて残す。

nativeの比較には既存の`benchmark_incremental_builds.py`に加え、新しい
`scripts/benchmark_smart_build_matrix.py`を使い、clean・変更なし・
本体・API・複数module・async・runtime依存・cache不整合の8ケースを区別する。
cleanは新しいCargo targetでありregistry/OS cacheまでcoldという意味ではない。
同profile・lock・target条件で複数回測り、Fresh/Dirty、容量、出力動作も確認する。
release設定は変更しない。未計測・遅いケースも結果に残し、ローカルの短縮から
GitHub Actions全体の時間・課金削減を推測しない。

新matrixは同じruntime/source pathの専有copy、同seed lock、構成別empty targetで
ordinary devとunitsを3回以上比較する。runtime copyのworkspace.membersだけをruntimeへ
絞った事実とmanifest/hashを記録し、元runtimeは変更しない。corrupt caseでは同じoutの
Cargo.lockを不正化し、parse拒否・旧latest/exe維持・元lock復元・回復buildを要求する。
Cargo artifact Freshとsource CAS hitを分け、全旧exeのhashと実行も各成功後に確認する。
`--prune-owned-targets`は完了済み専有targetだけを整理し、logs/out/旧exeを残す。
11個のpure safety testsは既存必須CI policy discoverで実行する。

最終gateは対象parser/module/diagnostic回帰、High/保存Low/native、共有cache/世代失敗回帰、
fmt/clippy/full suite、CI投影・必須security gate、実行後の独立auditとexact-head CI。
以前のmain CIで観測されたB1 token bootstrapの504は未解決事項であり、今回のTokio更新や
CI成功だけを原因解決の証拠にしない。再現・stage観測なしに期限を延ばしたり直列化しない。

ローカルのfmt/clippy、parser/module/overlay/Unicode/CRLF/改行なし回帰、CI policy160、
CSRF deadline inventory37、利用者CI generator7が成功。ネットワーク制限によるsocket bindの
`Operation not permitted`で最初のworkspace検査は失敗し、ログを保持した。
network許可を付けた全workspace検査はexit 0、670.09秒で完了した（既存ignore維持）。
この時間は同条件のbefore/after性能比較ではない。8ケース×ordinary/units×3回のmatrixも
完了し、48成功exeと6回の不正lock拒否・復元を確認した。100関数のfixtureでは変更なしが
約16%速い一方、編集は約9–21%遅く、分割の追加費用があるためopt-inを維持する。
独立auditとexact-head CIの最終状態はPRで読み戻し、未完のgateを成功と扱わない。

## 開発環境の容量

ユーザーの明示許可に基づき、ビルド・テスト・計測の区切りと重い処理の直前に
workspaceとtemporary filesystem双方の空き容量を確認する。再生成可能な自担当cacheを
先に整理し、source・未commit差分・raw計測log・比較用compiler・必要な証拠は残す。
稼働中のCargo targetや常設lockは削除しない。cache削除後の計測はcold/warm条件を明記する。
