# CI改善の判断と測定（2026-10-20）

0.2.0の凍結候補とは別の、次期開発用変更。[段階別の支持範囲](ci-levels.md)と[nextest比較](nextest-evaluation.md)を参照。検査のtimeout、assert、seed、feature、公開成果物のコンパイルprofileは変えていない。

## 実測したボトルネック

基準は[run 39415520849](https://github.com/disnana/Nagi/actions/runs/39415520849)、HEAD `467f8c9c791b4e98d1d33e15f8b9fc5515dafe0a`。GitHubの完了job/stepのstarted_atとcompleted_atを使った単一runの観測であり、改善後との同条件比較ではない。

| 観測対象 | 経過時間 |
|---|---:|
| 最初のjob開始から最後の検査job終了 | 3,463秒 |
| 成功した21 jobの実行区間合計 | 364.53 runner-minutes |
| Linux workspace `cargo test --locked` | 1,492秒 |
| CLI回帰 Linux / Windows / ARM Mac / Intel Mac | 727 / 1,415 / 1,041 / 1,538秒 |
| Intel Macのpackageとregression job合計 | 96.30 runner-minutes |
| Windows回帰のcache復元 | 約3.53 GB、約160秒 |
| Intel packageのcache保存 | 203秒 |

runner-minutesはOS料金倍率・課金丸めを適用していない実行区間の合計。請求額、AI処理時間、runner queue待ち時間ではない。job開始前には依存待ちも含まれるため、それをqueue時間と断定しない。Linuxでは同じcache keyの予約競合も観測した。数値を一般的なSLAやnextestの速度効果に転用しない。

## 採用した構成

- 変更判定→release-plan→ci-planの後、独立したRust包括検査、OS別package/回帰、browser、IDEを別runnerで並行実行する。重いnativeテストを同じrunner内で一斉起動せず、既存のport・一時directory・Cargo lockの分離と検査を維持する。
- L1は限定したfront-end変更だけ。未分類やruntime/security/control-plane変更はL2。main/週次は3 OS、明示RCと正式配布はIntelを含む4 OS。Intelを外すことによる通常CIの削減は選択変更であり、各テスト自体が高速化したという意味ではない。
- Readyは全選択jobを確認する。公開直前に同じ計画とsource SHAを再確認する。PR・手動RC・tagから公開しない。既存のversion/CHANGELOGによる公開判定を維持し、mainの公開runを後続commitで取消しない。新しいmerge承認機構を設定済みと仮定しない。
- 失敗時にも判定が必要な6 jobは `always() && !cancelled()` とする。[GitHubの取消仕様](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-cancellation)ではjob条件がtrueなら取消後も継続するため、公開jobも取消時には拒否する。stepの後片付け用 `always()` は維持する。既に外部で受理された公開の取り消しや、旧runへの遡及は保証しない。
- 外部Actionを確認したcommit SHAへ固定。checkoutの資格情報を残さず、診断artifactは7日、配布候補は14日の明示保持期間（既存の個別指定は維持）。ソース・秘密情報・私有鍵をcacheへ追加しない。
- 歴史的なBrowserLogin追加観測とeditor-stage観測は手動専用。必須のruntime/実browser検査は本体CIに残す。editor-stageは旧source束に限定した観測で、現在のcompiler変更を受け入れる汎用性能検査ではない。元のsource bindingは維持し、不一致なら拒否する。

## cacheとRust runnerの比較

| 手段 | 今回の判断と理由 |
|---|---|
| 既存actions/cache | restore/saveを分離。既存の重いkey/復元対象は維持し、全通常step成功後のmainだけ保存。PRによる大量保存とpost-step予約競合を減らす。復元自体の費用は残るため、次の実測で判断する |
| L1専用Rust cache | `nagi-critical-v1`のRust build/dependency領域を成功PRに保存。L1はmainで実行しないため、この例外が必要。GitHubのPR ref scopeと別keyを使い、公開producerはこの領域を参照しない |
| Swatinem/rust-cache | workspace crateを既定で除外する方針などが現行native-target/cfg別buildと異なる。置換による再ビルド・保持量を未測定のため今回は採用しない |
| sccache | incremental Rustやlinkの制約があり、通信/復元費用を含む利得が未測定。導入済み・有効と扱わず、公開profileも変えない |
| cargo-nextest | checksum検証済みの版で独立評価。二つの既存compiler targetでinventoryとignored状態を照合し、build・列挙・交互順のwarm実行を別計測。libtest出力依存の診断やsocket/native全suiteはまだ移行しない。doctestはCargoで維持 |

既存dev/testのincremental無効と、releaseのopt-level=3、lto=false、codegen-units=1、panic=unwindを維持する。debug情報やlinkerの一括変更は証拠なく行わない。`check`は型検査、`build`は配布物、`test`は実行検証として目的を分ける。異なるRUSTFLAGSの診断buildを同一成果物とみなして省略しない。

## Python

7 workflowのPython処理を調査。pip依存導入は本体Linuxとwebsiteの2 jobだけで、requirements別のpip cacheが既にある。他は主にstdlib unittest/診断/配布検証。利用可能な前段run `39358061068`（HEAD `1229d412d535c3bb4b87ae27b3959fc01cec2eff`）ではsetup-pythonが0〜1秒、pip導入3秒。build_examplesの277秒は主にnativeコンパイルで、Python起動の遅さではない。

この根拠ではuvやpytest-xdistを追加しない。Pythonのsocket/native検査の並行安全性も未証明であり、不要な依存を増やさない。tests/requirements.txtは版の範囲指定、websiteは直接依存の固定で、いずれも全transitive依存のhash lockではない。cacheを依存の固定と同一視しない。

## 確認方法と未確認

repository rootで `python -B -m unittest discover -s scripts/ci -p 'test_*.py'` と `python -B -m unittest discover -s scripts/ci -p 'test_nextest_evaluation.py'` を実行する。必須CIは従来のsource/mutation guardsも実行する。許可した配線の逆変換だけで元の検査commandへ戻ること、不正なpin/matrix/公開条件・command削除を拒否することを維持する。歴史的hashの新しい値への置換ではない。

実Actionsの[L1 run 39511768690](https://github.com/disnana/Nagi/actions/runs/39511768690)（HEAD `44b9d13`、一時PR #137は未マージで終了）では、lexerのcommentだけを変更し、重要Rust検査・browser・4 IDE検証・Readyが成功した。wallは584秒、非加重job実行区間合計は31.38 runner-minutes。包括検査はこの選択では実行せず、上の基準runとは作業集合が異なるため、同条件の高速化率ではない。

4 OSの[nextest実測](nextest-evaluation.md#2026-10-20の実測)では、この軽い2対象は従来Cargoより約1.6〜2.0倍長くかかり、必須runnerは置換しない。最終HEADのL2結果・wall/job・cache観測はPRの検証欄で同一HEADを明記する。新しいL3配線の手動4 OS実行と、実プラットフォームでの取消は未確認であり、ローカルの厳格なmatrix/拒否条件検証とは区別する。L3の公開前必須ゲートは維持し、未完了から公開しない。総課金削減額や一般的な性能保証は示さない。

参照（2026-10-20確認）: [GitHub cache境界](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)、[cache restore/save](https://github.com/actions/cache/tree/0057852bfaa89a56745cba8c7296529d2fc39830)、[rust-cache](https://github.com/Swatinem/rust-cache)、[sccache](https://github.com/mozilla/sccache)、[uvのGitHub案内](https://docs.astral.sh/uv/guides/integration/github/)、[nextest文書](https://nexte.st/docs/)。
