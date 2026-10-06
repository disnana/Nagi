# private Task bridgeの検証記録

[結果と限界](../../../docs/internal/task-bridge-stage1-results.md)。`provenance.json`は保存物と最終入力のSHA-256。01〜07は実行時のsource snapshotであり、現行runtimeへ適用するコードではない。

- 01: API未定義のcompile RED。02:通知だけでreceiveを完了するnative RED。03:受取後に故障を解除するnative RED。
- 04:補正前15群のGREEN。05:元Errorを読むAPI未定義のcompile RED。06:元Errorを消したnative RED。07:元Error保持後16群のGREEN。
- runner-location-red:先行runnerのcanonical path不一致を既存checkerの元行oracleで検出。
- compiler-final-red:元行修正後のLowコメント誤りも保存。`fixtures-before-comment-fix`は誤った`//`コメントを含む入力。最終`*-corrected`は56入力中6一致・50未達、exit 1。parse拒否をchecker契約の成功へ数えない。
- `green-runtime-lib.log`はnetwork権限なしでsocket 35件がPermissionDeniedになった旧試作の実行。失敗を消さず保存し、同source・network付き205成功を別logにした。最終は`green-runtime-legacy.log`の206成功。

このdirectoryは速度・allocation・Future sizeのbenchmark結果ではない。未実装のTask契約を除外して完成と説明する記録でもない。
