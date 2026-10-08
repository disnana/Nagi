# 廃止したHTTP APIの移行

未リリース0.2.0 SF01では、`@get/@post/@put/@delete`、`serve(Db, port)`、`Html/html`を廃止しました。High・保存Low・手書きLowのcheckerが`SF01 migration`で拒否します。旧APIの併存やpolicy省略のescape hatchはありません。

公開済み0.1.xの旧動作はその版のリポジトリで確認してください。開発sourceでは[HTTP入門](http.md)から明示Policyを使い、[0.2.0移行ガイド](migration-0.2.0.md)でhandler・path/query・JSON/body・エラー変換の対応を確認します。

`/health`は明示public routeに登録してください。組み込み`/stream`と`/ws`の注入はありません。有限bytes応答は利用でき、任意streaming/WebSocketとactive HTMLは未実装です。typed HTMLはSF04の別工程で、旧HTML画面の同等移行はまだ完了していません。
