# Resultとtry：入力から関数境界まで

[English](README.en.md)

Nagi 0.2.1で動く、小さなコンソール教材です。`main.nagi`は各行の近くにPythonとの対応とNagiの意味をコメントしています。`clean.nagi`は同じ考え方を短く書いた版です。`companion.py`も同じ入力、計算、回復動作をPythonの例外で実装しています。

リポジトリルートから実行します。

```sh
nagic check --project test-nagi-code/result-try-tutorial
nagic run --project test-nagi-code/result-try-tutorial
```

clean版は`nagic run test-nagi-code/result-try-tutorial/clean.nagi`で個別に実行できます。

`21`なら`21`、`42`、`完了`を表示します。`abc`なら数値変換のErrをmatchして理由を表示し、入力を直して再実行する案内を出します。`-1`ならこの教材が作ったErrを同じ場所で回復します。

## 何が起きるか

`parse_i64`が返す`Result[i64, Error]`は、成功値`Ok(i64)`か失敗値`Err(Error)`を表す型です。ResultはNagi独自の発明ではありません。Nagiでは成功・失敗を戻り値として明示し、`try`と`match`で扱います。

`make_quote`から`parse_amount`を呼びます。`parse_amount`の`try parse_i64(text)`は、成功なら整数を`amount`へ入れます。失敗ならErrを`parse_amount`の呼出元である`make_quote`へ返します。`make_quote`内の`try parse_amount(text)`がそのErrを受け、さらに`make_quote`の呼出元（`main`）へ返します。教材の`main`はそのResultを`match`し、Errをここで表示して回復します。`main`の戻り値は`Result[unit, Error]`なので、同じ式を`try`で書くclean版ではErrがプログラム入口まで伝播し、実行時エラーとして報告されます。

`Quote`は2つの整数フィールドを持つclassです。`make_quote`は成功時に`Ok(Quote(...))`を返します。Nagiのclassはこのようにデータをまとめられます。ここではmethodやfieldの再代入は使いません。

Python版は`int(text)`が`ValueError`を送出し、捕捉しなければ呼出し元へ伝播します。同じく`main`で捕捉し、同じ案内を出します。Nagiの`try`はPythonの`try/except`ではなく、ErrをResultの戻り値として呼出元へ返す演算子です。

## 次に読む

- [Result・try・matchの仕様](../../docs/error-handling.md)
- [文法の早見表](../../docs/syntax.md)
- [HTTP/JSON/SQLiteをまとめて使うResult API](../result-api/README.md)
- [JSONとSQLiteの在庫集計アプリ](../application-examples/stock-report/README.md)
- [HTTPとSQLiteの通し教材](../../docs/task-api-tutorial.md)
