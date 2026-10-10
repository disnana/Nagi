# SF05 release-plan test-mock follow-up evidence

このpacketは親agentが保存したlocal RED/GREEN記録と、PR #106 run #525のread-only GitHub Actions結果を結び付ける。元のSF05 commit/treeは `47dfc09ab36e553e45f8b49b1b3eba5ee16ddc7b` / `17a53331a7fb31532042f0c01419134ed788ef83`。この記録時点でworktree source diffは親の `scripts/releases/test_verify.py` 1ファイルだけだった。

`source/original-test_verify.py` はHEADのtest sourceとSHA-256一致する。`source/fixed-test_verify.py` は親の作業sourceのsnapshot、`source/test_verify.patch` はHEADとの差分である。17個の `test_*` method ASTが元と一致し、production `scripts/releases/verify.py` のworktree/HEAD bytesも一致する。補正はtest mockが `sqlite.literal(` の行を見つけるようにし、このgate fixtureの匿名 `?` 二つと `bind_i64` 一つを判別する。任意SQLを解析するmockではない。

`local-tests/` のraw filesは `/tmp/nagi-sf05-distribution-mock-fix/` からbyte-exactでコピーした。REDは17件中14成功・3失敗、GREENは17成功・0失敗・0 ignored。元のlocal command receiptは手元の記録に含まれていないため、ここでは実行コマンドを推測していない。このpacketを作る際にテストは再実行していない。

`public-ci/run-38031503012-release-plan-job-114153222296.raw.log` は公開run #525の失敗jobをGitHub Actions read-only log APIから取得したraw log。`public-ci/ci-readback-initial.json` はPR/tree/run identityと取得時のsummary、`public-ci/ci-jobs-latest.json` は2026-10-10 06:46:23 UTCのjob status readbackである。公開headは `5e992b6cd775912b6886c0b6763a4364642c5132`。`release-plan` は3/116 tests failed、`changes` は成功、Linux jobは `cargo test --locked` 実行中、package jobsはskipだった。

親のmock correctionは公開PRへまだ反映されておらず、そのcorrected sourceに対するpublic CI resultはない。独立reviewも未完了。したがって元PR runの失敗は未解決の公開gateとして残る。この資料はtest-mock差分の記録で、SQL parser、production verifier、SF05全体、4 OS、release readinessの成功証拠ではない。
