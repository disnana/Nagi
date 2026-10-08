# Actor CI同期修正の独立レビュー

Reviewer: `/root/actor_ci_review`、GPT-6.1 Sol / High、read-only。実装者から分離し、同じ問題の並行実装は行っていない。対象はb4c8f83の失敗原ログ、Actor source/Docs契約と、その上の`runtime/src/actor/tests.rs`の7行追加差分。テスト再実行・production編集はreviewerが行っていない。

初回の静的確認では、ReplyPortがREPLY_LOSTを通知した時点で旧mailboxがまだopenとなる順序を確認した。readyは次世代barrierではなく、現在のopen senderを待つ。旧mailbox閉鎖前に次callを送れるというtestの同期不足であり、公開lifecycle違反の証拠ではない。

修正後の独立確認では、失敗させる世代のsenderを事前に保持し、closed()後にreadyへ進む差分は妥当。全original assert、既存call/ready deadlines、2-worker実行、production動作を維持する。runtime source配下の変更はtests.rsだけ。追加のcorrectness/public-contract blockerなし。

読み戻した原ログはfocused1 passed、runtime lib219 passed/1既存ignored、consumer1、Task3、doctests10、clippy成功。旧失敗logのSQLite73成功とd723/b4のtree一致も照合した。これは最終headのCI成功の代用ではない。

記録文の2点を修正指摘し、親が反映した: closed()はmailbox閉鎖の確認であり、全destructor終了やactual joinではない。lifecycle.rs:413はjoin結果受取で、failure handlingはactor.rs:622。詳細は[同期修正の記録](../actor-ci-sync.md)。
