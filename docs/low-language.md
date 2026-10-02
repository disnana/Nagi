# HighとLow

Highは字下げでブロックを書く`.nagi`、Lowは波括弧と`;`で書く`.low`です。どちらもNagiの言語です。普段はHighで書き、関数の実装を差し替えたいときにLowを使えます。

## HighをLowへ変換する

次を`app.nagi`に保存します。

```nagi
def score(value: i64) -> i64:
    return value * 2

def main():
    print(score(7))
```

```sh
nagic run app.nagi
nagic lower app.nagi
```

実行結果は`14`です。`lower`は型・所有権を検査し、`build/app/generated.low`にLowを出力します。Lowでは関数を`fn`で宣言し、ブロックを`{ }`で囲みます。生成された変数には`let`と推論した型が付きます。

## 手書きLowで関数を差し替える

次を同じディレクトリの`native.low`に保存します。

```low
@replace generated::score
fn optimized_score(value: i64) -> i64 {
    return value * 6;
}
```

```sh
nagic check app.nagi --native native.low
nagic run app.nagi --native native.low
```

今度は`42`と表示されます。High側の呼び出しは`score(7)`のままで、実行する本体がLowの`optimized_score`に置き換わります。`@replace generated::score`が差し替える対象を指定しています。

差し替え先は、引数の数・型、戻り値の型、asyncかどうかが元の関数と一致する必要があります。対象が存在しない場合や、同じ関数を複数回差し替える場合はエラーです。変更できるのは関数全体です。

`generated.low`はコマンドを実行すると再生成されます。変更を残すには`native.low`へ書いてください。`--native`は差し替えに加えて通常のLow関数も追加でき、Highからその関数を呼べます。設定を保存する方法は[プロジェクト設定](projects.md#rustと手書きlowを追加する)にあります。

## Lowの文法

型、所有権、借用、Resultの扱いはHighと共通です。次はResultを処理する関数の例です。

```low
fn number_or(text: view[str], fallback: i64) -> i64 {
    match parse_i64(text) {
        case Ok(number) { return number; }
        case Err(_) { return fallback; }
    }
}
```

`Ok`と`Err`の両方が必要です。詳しくは[エラー処理](error-handling.md)を参照してください。

現在のLowは、型付きの変数、view、class（Lowではrecord）、関数、分岐、ループ、async、scopeに対応しています。生ポインター、メモリ配置の指定、手動の確保・解放、SIMD命令、unsafe構文、C ABIは未対応です。Rustの関数を呼ぶ方法は[ファイルのimportとRust連携](modules-and-rust.md)にあります。
