# RustでHTTPの基盤を用意する

既存のRust連携を使い、Axum 0.8とTokio 1.48でHTTPサーバーを組むプロジェクトです。Rustがlistener、route、リクエストの扱いを担当し、通常の同期Nagi関数が挨拶を選びます。組み込みの`serve`、`Db`、SQLite関数は呼び出しません。

このディレクトリで実行してください。`nagic`コマンドとRust/Cargoが必要です。初回のビルドではCargoが依存をダウンロードする場合があります。

```sh
nagic check
nagic run
```

リポジトリのルートからは次のように実行できます。

```sh
nagic check --project test-nagi-code/library-examples/custom-http
nagic run --project test-nagi-code/library-examples/custom-http
```

`check`はNagi側の宣言とcallbackの型を検査します。`run`ではRust側もコンパイルし、実装がその型に一致することを確認します。停止はCtrl+Cです。Axumが新しい接続の受付を止め、処理中のリクエストの終了を待ちます。

bind先は`127.0.0.1:8088`だけです。起動前にポートを変えられます。

```sh
NAGI_SAMPLE_PORT=8089 nagic run
```

PowerShellでは次のように指定します。

```powershell
$env:NAGI_SAMPLE_PORT = "8089"
nagic run
```

ポートは1〜65535の整数です。0、負数、65535を超える値、整数でない値は起動時にエラーになります。指定ポートが使用中の場合も起動エラーになります。

## routeを試す

サーバーを起動したまま別のターミナルで実行します。以下は既定のポートです。Windowsで`curl`がPowerShellのaliasになっている場合は`curl.exe`を使ってください。

```sh
curl http://127.0.0.1:8088/health
curl http://127.0.0.1:8088/hello/7
curl http://127.0.0.1:8088/hello/42
curl -i http://127.0.0.1:8088/missing
curl -i http://127.0.0.1:8088/hello/not-a-number
```

| リクエスト | 結果 |
|---|---|
| `GET /health` | 200、`ok` |
| `GET /hello/7` | 200、`Hello, Nagi! 7` |
| `GET /hello/42` | 200、`Hello from the Nagi callback! 42` |
| `GET /missing` | 404、`Route not found` |
| `GET /hello/not-a-number` | Axumの型付きpath extractorが400を返す |
| i64の範囲外の整数 | 同じextractorが400を返す |
| `POST /hello/7` | 405。このrouteはGETを受け付ける |

成功時のtext応答の末尾には改行があります。挨拶の違いから、Rustがpathのi64値を渡してNagiの関数を呼び出すことを確認できます。

body上限を試すには、次の任意のテストを使えます。Pythonで65537 byteを送信し、413応答を確認します。

```sh
python -c "import sys; sys.stdout.write('x' * 65537)" | curl -i -X GET --data-binary @- http://127.0.0.1:8088/hello/7
```

## この基盤で決めていること

`native.rs`に次の方針を実装しています。

- bodyの読み取りやhandlerの実行を同時に行うリクエストは最大8件です。追加のリクエストには`Retry-After: 1`付きの503を返し、アプリ側の待機キューには入れません。
- GETの未使用bodyを含め、すべてのbodyを65536 byte上限のAxum `Bytes` extractorで読み取ります。上限を超えるbodyには413、それ以外のbody読み取り失敗には400を返します。
- middlewareに到達してからbodyを読み終え、handlerが応答を作るまでに3秒の期限を設けます。期限を超えたリクエストには408を返します。
- 同期callbackはTokioのblocking poolで実行します。HTTP応答がtimeoutした場合も、callbackが終わるまでリクエストの枠を保持します。実行開始済みのcallbackをTokioが強制停止することはできないため、callbackは終了する処理にしてください。

組み込みHTTPの保護や`NAGI_HTTP_REQUEST_WAIT_SECONDS`は、このサーバーには自動では適用されません。このサンプルでは、リクエストheaderやidle接続の期限、接続数の上限、応答送信の期限、TLS、認証は設定していません。それらの方針はRust側の基盤の作者が決めます。終了しないcallbackはランタイムの停止も遅らせる可能性があります。ここではgraceful shutdownに別の期限は設けていません。

## 実装を差し替える

| ファイル | 担当 |
|---|---|
| [custom_http.nagi](custom_http.nagi) | adapterの宣言、`render_greeting`、`NAGI_SAMPLE_PORT`の読み取り |
| [native.rs](native.rs) | Axum router、上限、shutdown |
| [nagi.toml](nagi.toml) | 入口、Rustソース、依存のversion |

橋渡しの`fn[i64, str]`はRustの`fn(i64) -> String`になります。`extern async def run_server(...) -> Result[unit, Error]`は`Result<(), nagi_runtime::Error>`を返すRustの非同期関数になります。生成するアプリは、実行と橋渡しのError型に`nagi-runtime`を使います。

Nagi側の挨拶の方針を残し、Rustのrouterを差し替えたり、別のRust基盤を追加したりできます。crateはmanifestの`[rust.dependencies]`で指定します。この例が渡すのは通常の同期関数pointerです。非同期callback、変数をcaptureするclosure、コンパイラの新しいAPI、独自のHTTP parserは使いません。

[English](README.en.md) · [Rust連携](../../../docs/modules-and-rust.md)
