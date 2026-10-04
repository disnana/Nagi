# 型付きJSONの見積API

[English](README.en.md)

`std.http.server`を使う、DBを必要としない見積APIです。リクエストのJSONを`QuoteInput`へ読み、数量と商品を検証して`Quote`を返します。型付きの`Config`はAppが所有し、各handlerは`shared[Config]`で参照します。

リポジトリのルートで起動します。Nagi 0.1.9と、Rust/Cargoのビルド環境が必要です。

```sh
nagic run --project test-nagi-code/application-examples/quote-api
```

既定の接続先は`http://127.0.0.1:8092`です。`NAGI_SAMPLE_PORT`でポート、`NAGI_SAMPLE_MAX_QUANTITY`で最大数量を変更できます。最大数量は1〜1000で、既定値は1000です。不正な設定は起動前にエラーになります。Ctrl+Cで停止します。

```sh
curl http://127.0.0.1:8092/health
curl -H 'Content-Type: application/json' \
  -H 'X-Request-ID: 123e4567-e89b-12d3-a456-426614174000' \
  -d '{"sku":"NOTEBOOK","quantity":2}' \
  http://127.0.0.1:8092/quotes
```

見積の成功時は200と`application/json`を返します。

```json
{"sku":"NOTEBOOK","quantity":2,"currency":"USD","unit_price_minor":1250,"subtotal_minor":2500,"shipping_minor":500,"total_minor":3000}
```

金額はUSDの最小単位であるセントを整数で表します。NOTEBOOKの単価は1250、送料は500、数量が5以上なら送料は0です。数量を1〜最大数量へ制限してから計算するため、この設定では`i64`の範囲に収まります。

| リクエスト | 応答 |
| --- | --- |
| `GET /health` | 200。`status`、`currency`、`maximum_quantity`を含むJSON |
| `HEAD /health` | GETと同じステータス・ヘッダーで本文なし |
| `POST /quotes` | 200。見積JSON |
| 不正なJSON、必須フィールドの欠落、余分なフィールド、型の違い | 400、`invalid_json` |
| 不正な、重複した、UTF-8ではない`X-Request-ID` | 400、`invalid_request_id` |
| 重複した、構文が不正な`Content-Type` | 400、`invalid_header` |
| `Content-Type`の欠落、または`application/json`以外 | 415、`unsupported_media_type` |
| 数量が範囲外 | 422、`invalid_quantity` |
| SKUが`NOTEBOOK`以外 | 422、`unknown_sku` |

`http.is_json_content_type`で`Content-Type`を判定します。大文字・小文字、前後の空白・タブ、`application/json; charset=utf-8`などのparameterに対応します。parameterは文字コードを切り替えず、本文はUTF-8で読みます。JSONは`sku: str`と`quantity: i64`の2フィールドだけを受け付け、数量の数値文字列や小数は受け付けません。両方の業務入力が不正なら、数量の検証を先に行います。

アプリが返す入力・業務エラーの本文は次の形です。JSONの内部エラー詳細を応答へ出しません。

```json
{"code":"invalid_quantity","message":"Quantity is outside the configured range"}
```

`X-Request-ID`は任意です。指定すると単一のUUIDとして検証し、正規化した値を見積の成功・アプリのエラー応答へ返します。サーバー側でIDを生成する機能ではありません。不正なIDはechoしません。`/health`ではこのヘッダーを処理しません。

存在しないrouteは404、`GET /quotes`など登録されていないmethodは405と`Allow: POST`です。

HTTP受信時の本文上限は4096バイトです。超過時はhandlerを呼ばず、413と`Connection: close`で拒否します。残りの本文を読み捨てずに閉じるため、TCP resetが起こり得ます。すべてのOS・client・送信条件で413を受信できることは保証していません。

これらのサーバー側の応答はアプリのmapperを通らないため、上記のJSON形式やリクエストIDのechoを保証しません。応答の生成自体が失敗した場合は500を返し、最後のfallbackには本文がありません。

`calculate`は独自の`Result[Quote, QuoteError]`を返します。`std.result.map_error`はJSONやHTTP処理の`Error`を`QuoteError`へ変換します。handlerはリクエストIDを保持し、`quote_response`の失敗時にIDを`ApiFailure`へmoveして、AppのmapperでHTTP応答に変換します。処理の各段階でIDをコピーする必要はありません。`/health`は組み込み`Error`を使い、`route_mapped`で別のmapperを登録しています。

`smoke.py`はHighと保存したLowの両方で使える検証関数です。正常な見積、送料無料の境界、型付きJSONの拒否、ヘッダーとUUID、共有設定の変更、サーバー側の制限、停止後のポート解放をlocalhostで確認します。共通の検証コマンドは[上のREADME](../README.md)を参照してください。

このサンプルは見積の計算だけを行います。見積の保存、在庫の確保、税、決済、認証は扱いません。標準サーバーはloopbackのHTTP/1.1に対応します。
