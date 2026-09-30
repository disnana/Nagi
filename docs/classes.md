# classと値のレイアウト

classは型定義です。classという表記自体がheap allocationを要求しません。

```python
class Point:
    x: f64
    y: f64
```

primitiveのみを含むclassはRustのCopy値型へ生成します。`List[Point]`はVec<Point>であり、Pointを個別のheap objectにしません。`examples/values.nagi`では二つのPointを走査し、合計10と型サイズを表示します。

StringやListを含むclassも本体は値ですが、フィールドの所有データにallocationがあります。この区別を保つことで、layoutとコピーのコストを考えやすくします。

0.1ではfieldと名前付き生成のみです。method、interface、trait、継承は未実装です。compositionとfree functionを使用します。借用fieldは寿命パラメータの仕様が未確定なので拒否します。

生成したlayoutは同じRustビルド内でHigh/Lowに共通ですが、Rustの通常struct layoutです。C ABIで安定したlayoutを宣言したものではありません。
