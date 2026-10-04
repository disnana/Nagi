# 手書きLowの注文見積もりCLI

JSONを1行読み、数量と整数価格を検証し、小計・割引・合計をJSONで返します。入口の`main.low`は`quote.low`をimportします。どちらも手書きLowで、Rustアダプターは使いません。

リポジトリのルートから検査して起動します。`run`にはRust/Cargoが必要です。

```sh
nagic check --project test-nagi-code/low-examples/order-quote
nagic run --project test-nagi-code/low-examples/order-quote
```

起動後、次のJSONを1行で入力します。

```json
{"customer":"東京","items":[{"product_id":1,"quantity":3,"unit_price_cents":2500},{"product_id":2,"quantity":5,"unit_price_cents":500}]}
```

出力:

```json
{"customer":"東京","lines":2,"units":8,"subtotal_cents":10000,"discount_cents":500,"total_cents":9500}
```

価格は最小通貨単位の整数です。小計が10,000以上なら5%を割り引き、割引の端数は切り捨てます。入力は1〜1,000行、数量は1〜1,000、単価は0〜1,000,000です。商品IDは正の整数、顧客名はUTF-8で1〜80バイトです。`lines`は入力行数で、同じ商品IDも別の行として合算します。この上限で小計は最大1,000,000,000,000となり、`i64`の範囲に収まります。

`quote.low`のrecordがJSONの形を定め、`QuoteError`がJSONの読み取り失敗と各検証失敗を区別します。`main.low`はResultをmatchし、不正な入力の理由を標準エラーに表示して終了コード1を返します。正常時は標準出力にJSONを1行だけ返します。

ローカル変数は`let subtotal = 0;`のように型を省略しています。recordのフィールドと関数の引数・戻り値には型を書きます。`view(text)`や`view(order.customer)`は文字列を読むための借用です。`Order`は`calculate`へ移し、計算後にその`customer`フィールドを出力用`Quote`へ移します。LowでもHighと同じ型・所有権検査を受けます。

この例の明細recordは整数フィールドだけで作り、`for line in order.items`で値をコピーして走査します。文字列などの非Copyフィールドを持つrecordも、forで読み取り専用に借用できます。非Copy要素のindex取得と、record全体のviewはまだ未対応です。

`smoke.py`は実行ファイルに23ケースを渡し、JSONの値・整数型・終了コードを検査します。正常値、日本語、割引の境界と端数、上限値、空配列、不正なJSONや型を含みます。このディレクトリで実行します。

```sh
nagic build
python3 smoke.py --executable build/native-target/release/nagi-main
```

Windowsでは実行ファイル名に`.exe`を付けてください。`NAGI_NATIVE_TARGET_DIR`でビルド先を指定した場合は、その場所の実行ファイルを渡します。

[English](README.en.md) · [HighとLow](../../../docs/low-language.md) · [JSON](../../../docs/json.md)
