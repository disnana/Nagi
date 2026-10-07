# Task/spawn: #94統合後の原ログ

対象main: `97b7242c2172c1d0c701a7b662294e4dbe268abf`。Taskの実装は0.1.11公開済み。このartifactは#94後のTask統合回帰・同保証費用の再測定を保存する。過去のS1/S2/公開時検証と今回の実行を区別する。[結果](../../../docs/internal/task-spawn-post94-results.md)、[引継ぎ](../../../docs/internal/handoffs/2026-10-07-task-spawn-post94.md)を参照。

`commands.md`にコマンドと環境、`logs/`に原ログ・148契約の段階/元行レポート、`cost/`に今回の生成/手書きRust・保存Low・raw JSONL・比較集計を保存する。`review.txt`は独立Sol Highの所見と修正後の読戻し。`provenance.json`はbase、production/変更sourceとartifactのSHA-256を持つ。現在の資料自身のcommit SHAは自己参照させない。binary/cacheは含めない。原ログと生成Lowの末尾空白・改行を変更しないため、このartifact内の`.gitattributes`はその2種類だけwhitespace検査を除外する。

測定はLinux x86_64 release、current-thread Tokio、calling-thread allocation、25 loop×7反復。同じTaskScope保証の手書きRustと生成RustでallocationとFutureが一致した。共有hostでbuild/回帰と同時進行したためtime生値から速度優劣を主張せず、buildログをclean buildや同条件compile時間比較と呼ばない。binary byteは集計に記録した。`cfg(test)` runtime費用oracleは別のlayout/workloadで、保持量・retire/high-water・fault causeを観測する。

通常suiteで費用用ignored 1を成功実行へ数えず、別の明示`--ignored`実行の1群を区別する。今回branchの4 OS/websiteの最新実行はPR ChecksとPR説明の読戻しに記録する。過去main/0.1.11の成功や今回のskip/filterを新規成功へ足さない。
