# 標準HTTP policyでBearer認証する

[English](README.en.md)

標準HTTP dispatcherがRust verifierを呼び、認証済み`AuthScope`だけを`/me` handlerへ渡す小さな例です。`/health`と`/restricted`は、それぞれ明示したpublic routeです。

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
| `/me`、ヘッダーなし・値が違う | 同じ401、`invalid credential`、`WWW-Authenticate: Bearer` |
| `/me`、設定した値と一致 | 200、`Hello, Nagi!` |
| `/me`、同名ヘッダーが複数・不正なUTF-8 | 400、`invalid security request` |
| `/restricted` | public handlerが業務上の403、`access denied` |

401/400は標準policyの失敗応答です。`/restricted`は認証policyの拒否ではなく、handlerが返す独自の業務応答です。`NAGI_DEMO_AUTHORIZATION`が空なら起動時にエラーになります。ポートは`NAGI_SAMPLE_PORT`で変更でき、停止はCtrl+Cです。

Rustの`native.rs` verifierは設定した文字列と一つのdemo credentialを比較し、subject 1の`VerifiedIdentity`を有限期限付きで返します。認証境界の配線例であり、本番token format、署名、audience、期限検査、失効照会、ユーザー管理を実装していません。実運用には審査済みtoken verifierが必要です。
