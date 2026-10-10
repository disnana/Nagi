# Disk source読み込みの保持本文を抑える内部改善

[English](compiler-source-read-retention.en.md) · [pipeline](compiler-pipeline.md)

2026-10-10 UTC。main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57` をbaseにしたsource-only branch `fix/compiler-source-read-retention-pr`。新しい公開制限・言語仕様ではない。SF05 `47dfc09`、既存Task/SQLite/IDE worktreeのsourceは変更しない。
検証結果と未確認範囲は以下に要約する。詳細な原証跡はこのsource-only branchには含めず、元の固定検証記録へローカル保全している。source patchのSHA-256と検証範囲は以下に記載する。

## 変更と維持する契約

従来はdisk sourceを全文UTF-8として保持した後、load単位8,000,000 bytes、file単位2,000,000 bytesを検査した。新しいprivate readerは8KiB chunkでEOFまで読み、UTF-8を標準validatorで確認する。fileの保持本文は既存2MB以内、超過分はbyte数とUTF-8検査に使い、本文として保持しない。不完全なUTF-8 suffixは最大3 bytes。予約にはfallibleな標準APIを使う。

診断は完了したreadのI/O/UTF-8、aggregate8MB、per-file2MB/parserの順を保つ。不正UTF-8・本文超過後もdrainし、後続I/O errorを優先する。Interruptedを再試行する。error text/path/親import元行、通常成功本文を維持する。

| 境界 | 今回の扱い |
|---|---|
| High、保存Low、手書きLowの入力 | 既存2MB/file、8MB/loadを維持 |
| 内部生成Low | private64MBの経路を維持。保存して再入力したLowは通常入力の制限 |
| native Low | 既存の独立loadとappend。コンパイル全体の新しい合算capを追加しない |
| import | depth64/files128/cycle/duplicate/元位置の契約を維持 |
| overlay、include_text、manifest、外部Rust | 既存経路・制限を維持 |
| compiler/check/Low/Rust/runtime | parserの既存値・文言をcrate-privateに共有するだけ。新public API/依存なし |

保持本文の改善であり、read量・処理時間・OS read待機・全allocation・RSSの上限ではない。正常sourceの複製・AST、allocatorの丸め、変更中file・EOFにならない特殊file、物理OOMからの普遍的回復は保証しない。

## REDと検証

最初の固定review source patch SHA-256は `d57fefc492fcd3e204d31be8ef13c5e829f68bd605139bd1f45a6379e8711fc5`。全target clippyの追加test module配置警告を受け、同一test blockをsource.rs末尾へ移した。production itemとtest bodyのbytesは不変で、修正版source patchは `47e679b5a7b9aaebb71941376e44c72720169ee6e8917295a79de89f1f9113bd`。元patch/review/失敗を保持し、修正版の成功と混同しない。

初回REDは小さいprivate保持limit oracleで2 failed/1 passed。初回source snapshotを事前保存していなかったため、元editから事後復元した。復元binaryは保存済み原binaryとbyte/hash一致し、exit101。事後復元を事前取得として扱わない。

| Linux/Rust1.99での検査 | 結果と範囲 |
|---|---|
| 最終reader/source unit | 7/7成功。小さいlimit、UTF-8 carry、不正/後続I/O/Interrupted/EOF、8KiB chunk境界、診断順 |
| 新source_read_retention integration | 3/3成功。通常disk/overlay、High/Low import/check、不正UTF-8元位置 |
| compiler lib全体 | 135/135成功、filter0 |
| 既存frontend_contracts/modules | 13/13、32/32成功、filter0。既存depth/size/import/replacement回帰 |
| 上記2 harness内のnative CLI | 5 test function中、計9回のrunを実行。High/生成保存Low/手書きLowとLow replacement/Rust JSON・SQLite bridge。40 functionはnative runなし |
| fmt/clippy | 対象限定clippy成功。初回all-targetsは `items-after-test-module` で失敗、配置修正後のworkspace fmt/all-target clippyは成功 |
| 独立Sol High READ-BUDGET-R01 | 修正必須指摘なし。固定source/artifactとRED復元provenanceを照合、unit7/integration3と追加有限differential3を独立実行 |
| 独立READ-BUDGET-R02 | 同一test blockの移動・修正版patch・fmt/clippy原ログを照合して配置修正を承認。後続全target原ログも別追記で照合 |
| workspace no-fail-fast継続 | 一度だけ実行してexit0。98 Cargo target・1004 pass/0 fail/1既存ignored/filter0。graph_render子processの1 pass/8 filteredは別blockで、raw集計は99 block/1005 pass。下記初回HTTP失敗は未解決 |
| 既存fuzz-smoke | 既定seed305419896、mutation1000（parse拒否731/check拒否192/checked77）、bounded native16、panic0、exit0。coverage-guidedではない |

native runは生成binaryのbuildまたはcache再利用後に実行する経路。run回数からCargo再build回数は推測しない。子stdoutは既存testが捕捉・assertし、保存原ログはharness stdout/stderr。`check --native`をnative実行に数えない。既存2/8MB fixtureを実行したが、新しい巨大入力・負荷・資源枯渇実験を追加していない。

後続CI設定は既存4 OS package jobのcompiler検査へ `--test source_read_retention` を追加する。reader unitは既存 `--lib`、Linux全integrationは既存workspace testへ接続する。これは登録設定であり、remote CI実行・4 OS成功ではない。

初回workspace testはsocket bindの環境PermissionDeniedでruntime191 pass/40 fail/1 ignored、以後targetへ到達せず。修正版・loopback利用権限ではruntime230 pass/1 fail/1 ignoredとなり、既存 `oversized_body_declared_length_rejects_headers_and_incomplete_upload_promptly` がresponse readでConnectionResetを報告した。runtime sourceはmainと不変で、原因をreader変更と確定しない。元assert・並列度を維持して失敗を保存し、未到達target確認のため一度だけ `--no-fail-fast` で続け、上表の全target結果を得た。後続runtime blockは成功したが、この失敗を解決済みにしない。個別rerun/skip/oracle緩和はしていない。

全targetを実行した成功commandは保存したが、このreader差分では初回HTTP失敗を解消しておらず全回帰acceptanceは未達。4 OS CI、private64MBの実境界、perf/RSS、allocator failure、変更中fileは未確認。独立reviewを最新CI acceptanceの代用にしない。

## 引継ぎ

source、CI登録、JA/EN Docsをこの専用branchへ保存し、詳細な証跡は元の検証記録にローカル保全する。初回HTTP ConnectionResetは歴史的な失敗として保持し、統合reader headでの新しいCI結果が出るまで解消済みとしない。main snapshot `b85656478ea0db3bb27e398c91c228cc3847f074` にはPR #105/#106で統合されたHTTP観測・共通接続修正がある。今回のreader統合候補は既存のtarget-platform compiler commandへ `--test source_read_retention` を加え、現行HTTP/SF05のCI stepを維持するが、この統合候補のCI結果はまだない。現mainのeditor symbol helperはchild processを10,000msで打ち切る。失敗した#107 headの期限は5,000msで、`nagi`/`low` の2 symbol testが約5.02/5.01秒でSIGTERMになった。これはtest専用deadlineであり、製品の`checkTimeoutMs`既定15,000msとは別である。入力、assert、16MiB出力上限は変わらない。統合reader headの4 OS結果、64MB実境界、perf/RSS、allocation failure、変更中fileは未確認。SF07全体のDoS対策完了とは呼ばない。
