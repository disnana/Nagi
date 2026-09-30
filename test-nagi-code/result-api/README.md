# Resultの成功・失敗を分けるAPI

入力を読んで数値を2倍にする小さなAPIと、SQLiteから1件読むAPIです。`match`で成功・失敗を分け、エラーの応答や代替データへの回復を試せます。

## 起動する

リポジトリのルートで実行します。ビルドにはRust/CargoとCコンパイラが必要です。

```powershell
cargo build --release --locked -p nagic
.\target\release\nagic.exe check --project test-nagi-code/result-api
.\target\release\nagic.exe run --project test-nagi-code/result-api
```

Linux / WSLでは最後の2行の実行ファイルを`./target/release/nagic`に置き換えます。

既定のポートは8097、DBはメモリ内です。起動時に`id=1, name="notebook"`のデータを1件作ります。ポートを変える場合は起動前に`$env:NAGI_PORT = "8098"`を設定します。終了はCtrl+Cです。

## 呼び出してみる

別のPowerShellでリクエストします。

```powershell
Invoke-RestMethod 'http://127.0.0.1:8097/api/double?value=21'
Invoke-RestMethod 'http://127.0.0.1:8097/api/items/1'
Invoke-RestMethod 'http://127.0.0.1:8097/api/fallback'
```

応答は順に`{"value":42}`、`{"id":1,"name":"notebook"}`、`{"id":0,"name":"cached item"}`です。Linux / WSLでは同じURLを`curl`で呼べます。

| GETのパス | 結果 |
|---|---|
| `/api/double?value=21` | 200。文字列を整数に変換して2倍にする |
| `/api/double?value=oops` | 400。整数にできない入力 |
| `/api/double?value=-1` | 400。0〜1,000,000の範囲外 |
| `/api/items/1` | 200。登録済みデータ |
| `/api/items/2` | 404。DBには対象がない |
| `/api/items/0` | 400。IDは正の整数が必要 |
| `/api/items/1000001` | 404。`not_found`で範囲外を返す |
| `/api/fallback` | 200。DB失敗を捕まえて代替データを返す |
| `/api/db-error` | 500。DB失敗のkindを保って返す |
| `/api/internal-error` | 500。内部エラーを作って返す |

`storage.nagi`の`read_optional`は、**存在しない`optional_items`テーブルを意図的に読みます**。これで実際のSQLiteエラーを起こし、同じ失敗を「代替データで回復する」「失敗として返す」の2通りで扱います。500の応答は`{"error":"internal error"}`で、SQLや内部の理由は応答へ出しません。サーバーのログにはエラーの種類や詳細が出ます。

## 起動したAPIにsmokeを送る

サーバーを上の手順で起動してから、別のターミナルで実行します。Python 3の標準ライブラリだけを使います。

```powershell
python test-nagi-code/result-api/smoke_api.py --base-url http://127.0.0.1:8097
```

15リクエストのステータス・Content-Type・JSONを照合します。Python側はサーバーの起動・停止、DBの作成やファイル操作をしません。リクエストは読み取りのみで、同じサーバーに繰り返し実行できます。

## コードを読む

- [server.nagi](server.nagi)：ルート、`match`、`not_found`、`fail`、代替データへの回復。
- [storage.nagi](storage.nagi)：非同期DB操作。戻り値は`Result[Item?, Error]`。
- [models.nagi](models.nagi)：JSONとDBのデータ型。
- [nagi.toml](nagi.toml)：CLI・VS Codeで共通の入口設定。

[VS Code拡張0.1.2](../../editors/vscode-nagi/README.md)と最新版の`nagic`を使えば、`read_item`や`Item`にF12で移動できます。importの文字列からファイルも開けます。プロジェクト内の編集中ファイルを保存してから使ってください。

文法と制限は[Resultのエラー処理](../../docs/error-handling.md)にあります。nullableの値を取り出すmatchはまだありません。この例では`Item?`をHTTPへ返し、値がない場合の404への変換をルート側に任せています。
