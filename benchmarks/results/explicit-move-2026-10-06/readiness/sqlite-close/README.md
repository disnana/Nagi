# private SQLite close回帰の検証記録

[原因と修正](../../../../../docs/internal/sqlite-close-regression.md)。public runtimeに入らないcfg(test) adapterの回帰。公開Pool/Txの完成記録ではない。

- REDはtestだけの0051f7a: stock APIの未poll waiterがpermitを予約している状態でcloseがCloseTimeoutとなる。Pool Drop後のactual joinを確認してから失敗する。
- GREENはa488348: close後のidle退役を追加し、同じ期待で成功する。
- 最終sourceはf6bc74a。予期しないReady経路で完了Futureを再pollしない。全回帰90 suite・900件成功（追加回帰1件を含む）、failed/ignored 0。
- CI原ログは68d215b/run37454227747の失敗。これと最新headのCIを区別する。原CIのthread interleavingまでは同定していない。
- Solは最終sourceを独立読取レビューした。Cargoの実行担当はrootであり、レビュー自体を実行成功と数えない。

command、snapshot commit/tree、exit、raw hashはverification.json。原ログはbytesを加工せず保存した。新しいCIの最終状態はPR #87の最新Checksを参照する。
