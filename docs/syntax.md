# 構文

Highは空白による字下げ、Lowは`{ }`と`;`を使います。タブ、曖昧な字下げ、不明な文字、閉じていない文字列はlexer/parserが拒否します。コメントは両言語とも`#`です。

```python
def sum_values(values: view[i64]) -> i64:
    total = 0
    for value in values:
        total += value
    return total
```

関数の引数と戻り値は型を記述します。戻り値の省略は`unit`です。ローカル変数は推論できます。再代入では最初に決まった型を維持します。

対応する文は、代入、return、関数呼び出し、if/else、while、for、async with scope、spawnです。式には四則演算、比較、and/or、not、classの構築、フィールド参照、配列、index、await、tryがあります。classの生成は`Point(x=1.0, y=2.0)`のように全フィールドを名前で指定します。

`elif`、break/continue、import、classのmethod、継承、lambda、汎用の辞書literal、SQLブロック構文は未実装です。これらを複雑なparserへ一度に入れる前に、型・寿命・ネイティブ実行を検証する方針です。

`len(str)`はUTF-8のbyte長です。文字数を数える操作との区別が必要です。整数のrelease演算はRust backendの固定幅演算の挙動に従い、加算等のoverflowはwrapします。debug Rust側ではtrapする場合があります。checked/wrapping算術を言語として統一することは仕様確定前の課題です。
