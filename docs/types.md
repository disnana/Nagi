# 型と推論

[目次](README.md) · 初めて読むなら：[入門ガイド](language-guide.md)

変数は`count: i32 = 10`、関数の引数は`count: i32`、戻り値は`-> i32`と書きます。ローカル変数の型は省略できますが、決まった型を再代入で変更することはできません。

| 型 | 実装上の表現 | 0.1の状況 |
|---|---|---|
| i8/i16/i32/i64、u8/u16/u32/u64 | native整数 | 対応。異なる型の演算に暗黙の変換をしない |
| f32/f64、bool | native値 | 対応 |
| str、bytes | String、Vec<u8> | 所有値。strはUTF-8 |
| List[T]、[T] | Vec<T> | 連続格納。primitiveとCopy classの走査に対応 |
| view[str]/view[bytes]/view[T] | &str、&[u8]、&[T] | 非所有。寿命を検査 |
| T? | Option<T> | 対応。Noneには型の文脈が必要 |
| Result[T, Error] | tagged Result | tryによる伝播、Ok / Errのmatchに対応 |
| shared[T] | Arc<T> | share/clone_sharedによる明示的共有 |
| UUID、timestamp | u128/i64のnewtype | native表現。UUIDは明示parse/format |
| Map[K,V]、owned[T] | HashMap、所有値への型方針 | 型表記の足場のみ。完全な操作APIは未実装 |
| generic、function/async function type | 静的特殊化を目標 | 汎用genericと型注釈の関数型は未実装 |

`count = 10`はi64、`rate = 1.5`はf64です。型注釈がある整数literalはその範囲を確認します。現在の数値変換APIは、i8 / i16 / i32 / u8 / u16 / u32からの損失のない`i64(value)`と、i64から範囲を検査する`i32(value) -> Result[i32, Error]`です。任意型への汎用castはありません。

VS Code拡張0.1.4では、変数名にマウスを置くと推論された型を確認できます。たとえば`count`は`count: i64`です。関数の引数やcaseの束縛名も対象です。[エディターの操作例](editor.md)で試せます。

nullableは`missing: i64? = None`、値がある場合は`present: i64? = some(42)`です。`T?`は`Option[T]`の短い表記ですが、現在はmatchや汎用unwrap APIはありません。空配列は`values: List[i64] = []`と型を指定してください。

全値をboxingする設計ではありません。ただし、String、Vec、task、channel等の内部allocationがなくなるわけではありません。RustのモノモーフィズムとLLVM最適化を利用します。ユーザー定義generic関数やtraitの実装は今後の段階です。
