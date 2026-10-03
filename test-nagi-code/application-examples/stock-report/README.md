# 在庫集計CLI

JSONを1行読み、倉庫ごとの在庫数と予約数を集計します。入力の検証とエラーの型は`inventory.nagi`に分けています。Rustアダプターは使いません。

リポジトリのルートで起動し、続けてJSONを入力してください。

```sh
nagic run --project test-nagi-code/application-examples/stock-report/nagi.toml
```

```json
{"warehouse":"東京倉庫","items":[{"product_id":1,"on_hand":12,"reserved":3},{"product_id":2,"on_hand":5,"reserved":5}]}
```

アプリの出力:

```json
{"warehouse":"東京倉庫","products":2,"available":9,"reserved":8}
```

商品IDは正の整数です。在庫は0〜1,000,000、予約数は在庫以下とし、最大10,000件まで扱います。`products`は入力行数で、同じ商品IDの行も別のレコードとして合算します。倉庫名はUTF-8で1〜80バイトです。

不正な入力は標準エラーに理由を表示し、終了コード1を返します。`InventoryError`でJSONの読み取り失敗と業務上の検証失敗を区別し、CLIの入口で表示用のメッセージへ変換します。

`smoke.py`は実行ファイルに正常値、上限値、空配列、日本語、不正なJSONや型を渡し、JSONの値と終了コードを確認します。共通の検証コマンドは[上のREADME](../README.md)を参照してください。
