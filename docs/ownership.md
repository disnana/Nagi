# 所有権

str、bytes、List、所有fieldを持つclassは、関数へ渡すとmoveします。move後の名前の使用はcheckerで拒否します。Copy値はそのまま複数回使用できます。

```python
def use_name(name: str):
    print(name)

def main():
    name = "tp-li"
    use_name(name)
    # print(name) はmove後の使用
```

viewを作った所有値をmove・再代入・appendすることも制限します。viewの借用は字句scopeで保守的に追跡します。Rustのnon-lexical lifetimeと同等の精密な解析はまだありません。

部分fieldのmove、複雑な分岐・ループ、genericな借用のsoundnessをNagi checkerだけで証明していません。Rust codegenは安全なRustを出し、backendの借用検査にも通ったものだけを実行ファイルにします。`check`の成功と`build`の成功を区別してください。

所有権を推論する狙いは、アプリ開発者にlifetime記法を増やさず、コピーが必要な場面を示すことです。所有データをもう一つ保持する場合は`copy(view(data))`を記述します。
