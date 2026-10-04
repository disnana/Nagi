# AxumとNagiを組み合わせる見積API

[English](README.en.md)

HTTPの基盤をRustとAxumで作り、型・業務の検証と計算をNagiで書く例です。既存のRust連携を使い、Nagiの標準HTTPサーバーは使いません。

```sh
nagic run --project test-nagi-code/application-examples/axum-service
```

リポジトリのルートで実行します。この例はNagi 0.1.10で検証しています。nagicとRust/Cargoが必要です。接続先は`http://127.0.0.1:8097`で、`NAGI_SAMPLE_PORT`で変更できます。Ctrl+Cで停止します。

```sh
curl http://127.0.0.1:8097/health
curl -H 'Content-Type: application/json' -d '{"quantity":2}' \
  http://127.0.0.1:8097/quotes
```

見積は`{"quantity":2,"total_minor":2500}`を返します。単価は最小通貨単位の1250、数量は1〜100です。数量を検証してから計算するため、この範囲では整数のoverflowは起こりません。

| 担当 | 書くもの |
| --- | --- |
| Nagi：[main.nagi](main.nagi) | 入力と見積のclass、独自エラーenum、数量の検証、asyncの計算関数 |
| Rust：[native.rs](native.rs) | AxumのrouteとJSON extractor、HTTP statusへの変換、loopback listener、停止処理、非同期timer |
| Cargo：[nagi.toml](nagi.toml) | Axum・Tokioの依存設定 |

`POST /quotes`では、AxumがJSONを`QuoteInput`へ変換し、Rust handlerが`super::calculate(input).await`を呼びます。Nagiの`calculate`はRustの`pause`をawaitしてから`Result[Quote, QuoteError]`を返し、Rustが成功を200、業務上の拒否を422に変換します。JSONの変換にはNagiが生成するSerde実装を使います。

呼び出しは、Rustから生成されたNagiの名前付きasync関数を直接参照する形です。任意のasync callback値をNagiのextern引数へ渡す機能ではありません。Nagi側の型・所有権は`check`、Rust側の呼び出しとFutureの条件は`build`で確認します。関数名や型を変えたらadapterも合わせて変更してください。`map`はRust内のrouteやこの呼び出しを解析しません。

数量の範囲外はJSONの`invalid_quantity`です。不正なJSONはAxumが400、フィールド欠落・余分なフィールド・型の違いは422、Content-Typeの欠落は415を返します。これらのextractorエラーの本文はAxumの形式です。

このadapterではJSON本文上限4096バイトとCtrl+Cのgraceful shutdownを設定しています。Nagi標準HTTPの接続数・受信／handler／送信の期限・panicの500変換・停止期限は自動で付きません。認証、TLS、DB、panicからの回復も追加していません。非同期の待機を取り消しても、外部処理や状態更新が巻き戻る保証はありません。`pause`は非同期の往復を示すtimerで、DB操作ではありません。

`smoke.py`は実HTTPで正常な見積、数量の境界、JSON入力の拒否、エラー後の正常応答、不正なport設定、停止後のlistener解放を確認します。共通の実行方法は[サンプル一覧](../README.md)を参照してください。Linux・macOSの検証ではCtrl+Cによる正常終了を確認し、Windowsではテスト側がprocessを終了させます。
