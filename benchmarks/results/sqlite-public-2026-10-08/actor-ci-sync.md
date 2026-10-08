# 既存Actor回帰テストの世代終了待ち

SQLite公開作業の最終CIで既存Actor testの同期不足を観測したため、公開動作を変えずテストの待ち合わせを修正した。新しいActor機能・API変更ではない。

## REDと根拠

source `b4c8f83f39a7ff608a5fe623545819e06ea7571b`の[push run37714512662 / linux113107859076](https://github.com/disnana/Nagi/actions/runs/37714512662/job/113107859076)で、`business_error_preserves_owned_state_and_restart_is_one_for_one`が旧sourceの`runtime/src/actor/tests.rs:48`で失敗した。意図したhandler failureの後、`ready`から戻った次のcallが期待した`Ok(10)`ではなく`REPLY_LOST`になった。runtimeは218 passed / 1 failed / 1既存ignored、SQLite73件は成功。[失敗原ログ](logs/ci-b4-113107859076-push-failure.log.gz)を保存した。Actor production/testはこの時点でmainとの差分0で、同じtreeのd723961全CIと同headのPR workspaceは成功していたが、それをこの失敗の代わりにしない。

独立Sol High reviewerがsourceと契約を確認した。`ReplyPort::drop`は`REPLY_LOST`を通知するが、その時点でactor futureのmailbox receiverと外側`ActorStatusGuard`のDropが済んでいるとは限らない。別Tokio workerのcallerは通知を受け、旧receiverがまだopenの間に`ready`からその世代のsenderを取得できる。そこへenqueueした次のreplyは旧世代終了時に失われ得る。`ready`は現在のopen senderを待つもので、次世代指定のbarrierではない。

確認箇所はb4c8f83の`runtime/src/actor.rs:757`（reply Drop）、`:919`（handler Err）、`:1015`/`:1046`（sender/readiness）、`:622`（join後の失敗処理）、`runtime/src/actor/lifecycle.rs:413`（join結果の受取）、`:250`（新世代開始）、`:651`（completion前のinner future Drop）。`docs/en/actor-reference.md`のreadyと`docs/en/supervisor.md`のrestart説明にも、REPLY_LOST後のreadyが次世代を保証する契約はない。production違反の証拠ではなく、testが必要な世代のmailbox閉鎖を待っていなかった。

## 修正と検証

意図して失敗させるcallの直前にその世代のsenderを保持し、REPLY_LOST確認後に`sender.closed()`を待つ。旧mailboxの閉鎖を確認してから既存の`ready`と値のassertへ進む。閉鎖は全destructor完了やactual joinの確認ではないが、readyが旧senderを選ばないためには十分である。待機は既存の1秒というtest deadline値でboundedにする。元のassert、call/ready期限、parallel設定は変更しない。production・公開契約・依存は変更しない。

既存の失敗testそのものを同期修正して検査し、別の同形testで置き換えていない。[focused test](logs/actor-generation-barrier-focused.log)は1 passed、[runtime全体](logs/actor-generation-barrier-runtime.log)はlib219 passed / 1既存ignored、公開consumer1、Task3、doc-tests10で成功。fmtと[workspace all-target clippy](logs/actor-generation-barrier-clippy.log)も成功。[独立reviewの追跡確認](logs/independent-actor-ci-review.md)では追加のcorrectness/public-contract blockerなし。mailbox閉鎖とactual joinの記述を区別する指摘も反映した。最終headの4 OS/全回帰はPR #99本文とChecksへ記録し、このlocal結果から推測しない。

## main履歴とCI

main保護ruleset24311546の`strict_required_status_checks_policy=true`に従い、PR99の作業branchへmainの履歴を統合したのがb4c8f83。d723961と同じtree `a9638aa7349af774ace3799732e309308cc6a475`で、file変更はなかった。これはmain/公開PRのmergeではない。以降のテスト同期修正はこのCI失敗の原因に対する限定変更であり、SQLiteやActorの公開動作を変えない。
