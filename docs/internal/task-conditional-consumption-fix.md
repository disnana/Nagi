# Taskの条件付き消費と正常出口義務の修正

2026-10-07。ADR 012の全T正常出口await/discard契約に対する実装修正。実装baseは `f9b25782a8704bcf962f689d9103ab804f7ab3cf`。未リリースであり、merge・4 OS・releaseの完了を示す記録ではない。

`False and take(await task)`、`True or take(await task)`、環境変数が存在する `env(key, take(await task))` は、実行時に受取式を省略する。旧checkerは省略される側も無条件に検査し、そこでTask binding義務を消去していた。そのため正常scope出口を誤受理した。生成Rustの短絡・lazy fallbackは契約どおりだった。

`compiler/src/check.rs` の `conditional_expr` は評価側の型・所有権・Task action factsを検査してから、評価直前の省略側状態を既存 `merge_moves` で統合する。must-consumeのpendingはどちらかの経路に残れば維持し、moveはどちらかの経路で発生すれば後続使用を拒否する。対象はand/orのRHSと解決済みbuiltin envの第2引数だけ。LHS/keyの評価後にsnapshotを取るので、そこにあるawait/discardは有効な無条件消費になる。canonical resource/operation identity、ScopeId、alias転送、sealed planは既存経路を使う。emitter/runtime/API/評価順を変更していない。

この処理は既存statement branchと同じ保守的統合を使い、literalから分岐をfoldしない。一般viewの新しい生存factや一般Futureの機能を導入する修正ではない。checker内部の状態snapshot以外に、新しい生成Rustのclone・allocation・暗黙await/discardは追加しない。

## 回帰の入力と観測

`tests/task-handles/contracts.json` に38入力を加えた。全体は148入力・74 High/Low対。and/or、既知/未知条件、nested短絡、env/nested env、await/discard、unit/Copy/str/内側Result、move alias、条件付き消費後の再await/再代入を含む。negativeはparse/import拒否を成功にせず、checker段階・diagnostic fragment・元行を既存runnerで照合する。diagnosticの `# primary` は実際の拒否元行に付けている。

`short-and-saved.low` と `lazy-env-saved.low` は保存した未修正0.1.11 CLIのlower出力をfixture化したもの。Task/TaskFailureのcanonical metadataを含み、元Highを削除した後にも旧checkerが受理することを確認した。登録checkerが要求するprimary commentだけを追加し、生成された型・symbol・metadata・式は変えていない。対応Highも登録してpair完全性を維持する。生のlower出力と旧checkログは外部artifactに残す。

既存 `task_handles` harnessへ `support/task_conditional_consumption.rs` を登録した。実nativeで7実行経路をHigh・元Highを削除した保存Low・独立手書きLowそれぞれから実Cargo build/runする。LHS/keyのawait/discard、eager await後のResultをlazy fallbackへ渡す正例、env present/missingのstr結果、and/orのprobe短絡を確認する。adapterのreceipt/probe/fallback/scope完了event列と戻り値をassertする。条件付き評価を強制eager化した場合も失敗するoracleである。

`support/native_triple.rs` は既存oracleを保持したまま、既存のopt-in artifact環境変数がある時に三構文source、Cargo build stdout/stderr、native stderrも保存する。生成Rust・native stdoutの従来保存も維持する。必要なsource/原ログ/hashは親が[保存artifact](../../benchmarks/results/task-conditional-consumption-2026-10-07/README.md)へコピーし、binary/cacheはリポジトリへ追加しない。

## 実行結果と範囲

外部artifact directoryは `/tmp/nagi-conditional-task-fix/`。source・logの正確なhashは同directoryの `source-manifest.json` / `artifact-manifest.json` に保存する。warm cache `/tmp/nagi-container-flow-target` を使用した。

- 修正前の先行契約: `original-red.json` / `original-red.log`。134入力中114一致。追加20負例は全てparse/resolve後にchecker誤受理した。再awaitの4負例は既存move後拒否を保った。
- 修正後の契約: `final-contracts.json` / `final-contracts.log`。148/148一致。元Task/alias binding行のmust-consume、再使用行のmove後、再代入行の未受取診断を確認した。
- 入力登録: `input-registration.log`。148入力・74対、未登録/orphan入力なし。これはparse/check/native成功とは別の観測である。
- 新しいTask native oracle: `conditional-native-final.log`。既存Task harness内の1 test成功、他7 testはこのfilterでは未実行。三構文それぞれ7経路をnativeでassertした。source/Rust/build/nativeログは `native/explicit-move-task-conditional-*`。
- 既存対象検査: `targeted.log` のconstant_validation 10、explicit_moves 9、Task checker/業務fault/生成path等5成功。TaskのHTTP2件は最初のrestricted execでsocket bind PermissionDeniedとなった。network付き再実行は親の全workspaceへ委ね、途中の `targeted-network.log` を完了成功には数えない。
- socket不要の追加対象: `view-targeted.log` のcomparison_ownership 4、loop_ownership 13、static_callback_views 3、view_branch_rebinding 4、view_container_drop 1、view_flow_completion 3、view_flow_foundation 2が成功。
- `fmt-final.log`: `cargo fmt --all -- --check` 成功。`clippy.log`: `cargo clippy --locked --all-targets -- -D warnings` 成功。`git diff --check` と既存compiler corpus登録検査も成功。

全workspace、HTTP2件の再確認、4 OS、独立critical review、最終commit/tree、merge、release接続は親の担当で、この実装者の結果では完了扱いにしない。一般view-containerを条件付きで渡し、その後containerを再使用せずownerを移動する対照例は、親が旧CLIの実build/nativeで成功を確認した。NLL上のloan終了が許される経路であり、Task blockerや今回の追加変更には数えない。
