# Nagiでヘッダーと認証エラーを扱う

標準HTTP APIだけで動く、小さなBearer認証の例です。RustのadapterやDBは使いません。共有状態にデモ用の値を置き、非同期handlerで`Authorization`を読みます。

このディレクトリで実行してください。

```sh
export NAGI_DEMO_AUTHORIZATION='Bearer example-only-token'
nagic run
```

PowerShellでは次のように設定します。

```powershell
$env:NAGI_DEMO_AUTHORIZATION = 'Bearer example-only-token'
nagic run
```

別のターミナルから試せます。Windowsでは必要に応じて`curl.exe`を使ってください。

```sh
curl -i http://127.0.0.1:8089/health
curl -i http://127.0.0.1:8089/me
curl -i -H 'Authorization: Bearer example-only-token' http://127.0.0.1:8089/me
curl -i http://127.0.0.1:8089/restricted
```

| route | 応答 |
|---|---|
| `/health` | 200、`ok` |
| `/me`、ヘッダーなし・値が違う | 401 |
| `/me`、設定した値と一致 | 200、`Hello, Nagi!` |
| `/me`、同じ名前のヘッダーが複数・不正なUTF-8 | 400 |
| `/restricted` | route専用の処理が共通処理を上書きし、403 |

401には`WWW-Authenticate: Bearer`を付けます。応答には認証用の値や内部エラーを含めません。未設定なら起動時にエラーになります。ポートは`NAGI_SAMPLE_PORT`で変更でき、停止はCtrl+Cです。

固定値の比較はHTTP APIを試すためのデモです。本番向けのユーザー管理、トークン発行・失効、安全な認証方式は実装していません。

[English](README.en.md)
