# 型と推論

| 型 | 実装上の表現 | 0.1の状況 |
|---|---|---|
| i8/i16/i32/i64、u8/u16/u32/u64 | native整数 | 対応。異なる型の演算に暗黙の変換をしない |
| f32/f64、bool | native値 | 対応 |
| str、bytes | String、Vec<u8> | 所有値。strはUTF-8 |
| List[T]、[T] | Vec<T> | 連続格納。primitiveとCopy classの走査に対応 |
| view[str]/view[bytes]/view[T] | &str、&[u8]、&[T] | 非所有。寿命を検査 |
| T? | Option<T> | 対応。Noneには型の文脈が必要 |
| Result[T, Error] | tagged Result | tryによる伝播に対応 |
| shared[T] | Arc<T> | share/clone_sharedによる明示的共有 |
| UUID、timestamp | u128/i64のnewtype | native表現。UUIDは明示parse/format |
| Map[K,V]、owned[T] | HashMap、所有値への型方針 | 型表記の足場のみ。完全な操作APIは未実装 |
| generic、function/async function type | 静的特殊化を目標 | 汎用genericと型注釈の関数型は未実装 |

`count = 10`はi64、`rate = 1.5`はf64です。型注釈がある整数literalはその範囲を確認します。型変換は、損失のない`i64(i32値)`、範囲を検査する`i32(i64値)`等を明示します。

全値をboxingする設計ではありません。ただし、String、Vec、task、channel等の内部allocationがなくなるわけではありません。RustのモノモーフィズムとLLVM最適化を利用します。ユーザー定義generic関数やtraitの実装は今後の段階です。
