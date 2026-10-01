# HTTPとHTML

[目次](README.md) · 前：[入門ガイド](language-guide.md) · 次：[SQLite](database.md)

Nagiでは、`@get`などを付けたasync関数がHTTPの入口になります。classを返すとJSON応答、`Html`を返すとHTML応答です。まずデータを保存しない小さなサーバーから動かします。

## 1. サーバーを書く

次は完全なコードです。[examples/tutorial/http.nagi](../examples/tutorial/http.nagi)にもあります。

```nagi
class Greeting:
    id: i64
    name: str

@get("/")
async def home() -> Result[Html, Error]:
    return ok(html("<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><title>Nagi</title><h1>Hello, Nagi!</h1><a href=\"/greet/1\">JSONを見る</a></html>"))

@get("/greet/{id}")
async def greet(id: i64) -> Result[Greeting, Error]:
    if id < 1:
        return error("id must be positive")
    return ok(Greeting(id=id, name="Nagi"))

@post("/echo")
async def echo(req: Greeting) -> Result[Greeting, Error]:
    return ok(req)

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    return await serve(db, 8094)
```

`serve`の現在のAPIには`Db`が必要です。この例ではメモリ内SQLiteを開きますが、テーブルの作成や書き込みはしていません。`serve`はサーバーを起動し、リクエストを待ち続けます。

## 2. 起動して呼び出す

8094番ポートを空けて、リポジトリのルートから実行します。

```powershell
.\target\release\nagic.exe run examples/tutorial/http.nagi
```

Linux / WSL2では`./target/release/nagic run examples/tutorial/http.nagi`です。ブラウザーで[http://127.0.0.1:8094/](http://127.0.0.1:8094/)を開くと「Hello, Nagi!」を表示します。

別のPowerShellでJSON APIを呼びます。

```powershell
Invoke-RestMethod http://127.0.0.1:8094/greet/7
Invoke-RestMethod http://127.0.0.1:8094/echo -Method Post -ContentType 'application/json' -Body '{"id":2,"name":"sample"}'
```

Linux / WSL2では次のコマンドです。

```bash
curl http://127.0.0.1:8094/greet/7
curl -H 'Content-Type: application/json' -d '{"id":2,"name":"sample"}' http://127.0.0.1:8094/echo
```

GETの応答は`{"id":7,"name":"Nagi"}`、POSTは送った`{"id":2,"name":"sample"}`です。`/greet/0`は400のJSONエラーになります。終了は起動したターミナルのCtrl+Cです。

## 3. 引数と応答を決める

| 書き方 | HTTPでの意味 |
|---|---|
| `@get("/greet/{id}")`と引数`id: i64` | URLの`{id}`を整数として読む |
| 引数`req: Greeting` | requestのJSON bodyをclassへ読む |
| 引数`db: Db` | serveに渡したDBをhandlerへ供給する |
| pathにないprimitive引数 | query parameterから読む。例は[crud.nagi](../examples/crud.nagi)の`/query` |
| 引数`body: view[bytes]` | request bodyを借用byte列として読む |
| `Result[Greeting, Error]`と`ok(...)` | 成功時にJSON応答 |
| `Result[Greeting?, Error]`と`ok(None)` | 対象なしを404にする |
| `Result[Html, Error]`と`ok(html(...))` | 成功時に`text/html`応答 |
| `error("理由")` | 入力エラーとして400のJSON応答 |
| `not_found("理由")` | 対象なしとして404のJSON応答 |
| `internal_error("理由")` | 詳細を伏せた500のJSON応答 |
| `fail(problem)` | Errorの種類を保った応答。DBエラーなら500 |

HTTP handlerは`async def`で定義し、`Result[..., Error]`を返します。属性には`@get`、`@post`、`@put`、`@delete`があります。JSONのfield欠落、型の違い、不明fieldなどは入力エラーです。

`match await operation(...)`で失敗を分け、既定値を返して回復することもできます。[Result APIサンプル](../test-nagi-code/result-api/README.md)では、入力不正の400、対象なしの404、DB失敗の500、代替データを返す200を実HTTPで確認できます。

## 4. HTMLを別ファイルにする

HTMLが長くなったら、`.nagi`と同じディレクトリに`index.html`を置き、handlerの本体を次のようにします。

```nagi
@get("/")
async def home() -> Result[Html, Error]:
    return ok(html(include_text("index.html")))
```

`include_text`はコンパイル時にHTMLを実行ファイルへ埋め込みます。HTMLを変更したら再ビルドしてください。配布先にはHTMLファイルを置く必要がありません。

画面のJavaScriptから`fetch("/api/tasks")`のように同じサーバーを呼べます。追加・編集・削除とSQLiteを組み合わせた完成例は[タスク管理デモ](../test-nagi-code/web-demo/README.md)です。

## 現在のサーバーの範囲

Highの属性からAxumのroutingを生成します。HTTP/1.1、keep-alive、path parameter、型付きquery parameter、request body、JSON responseを実装しています。

標準の試験用endpointは`/health`、5chunkの`/stream`、echo WebSocketの`/ws`です。middlewareでrequest処理を2秒に制限し、bodyとWebSocket messageの上限を1 MiBにしています。DBや内部エラーは500などに変換し、詳細をresponseへ出しません。

HTTPの待機期限は既定で10秒です。接続直後の無通信、途中のヘッダー、応答後から次のヘッダーが完成するまでが対象です。少量ずつ送信しても期限は延びません。期限を過ぎた接続は閉じられるため、クライアントは必要に応じて再接続してください。処理中の応答、ストリーム、アップグレード後のWebSocketには、この待機期限を適用しません。

変更する場合は、起動前に環境変数`NAGI_HTTP_REQUEST_WAIT_SECONDS`へ正の整数を指定します。例えばPowerShellでは`$env:NAGI_HTTP_REQUEST_WAIT_SECONDS = "30"`、bashでは`export NAGI_HTTP_REQUEST_WAIT_SECONDS=30`です。ヘッダーと未使用keep-aliveの期限は共通です。同時接続数を128に固定する制限はありません。

現在はloopback専用です。HTTP/2、TLS、認証、任意middlewareのHigh宣言、deploymentの仕組みは未実装です。大きいclassのJSON streamingや汎用のHigh streaming構文もありません。JSON responseはclassをVec<u8>へencodeしてBodyへ渡します。
