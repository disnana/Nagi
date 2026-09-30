# メモリモデル

0.1の実装はnative値＋所有値＋非所有viewです。Rust backendが最後の借用・Send検査とdropを担当します。独自GCはありません。すべての値にatomic参照カウントを付ける方式でもありません。

| 値 | 主な保持・解放 |
|---|---|
| primitive、Copy class | stack/register/配列要素として保持 |
| str、bytes、List、所有class | moveで所有権を渡し、Rust dropで解放 |
| view | 元データの寿命内で借用。所有権と解放責任を持たない |
| shared | 明示Arc。clone/dropでatomic refcountが発生 |
| task/channel/DB job | ランタイムがallocationし、終了・キャンセル・dropで解放 |

request arenaは重要な設計候補ですが、Highへ暗黙に導入していません。bumpaloのscope内実験はあります。arena内にStringやFDを持つ値を置いた場合、一括メモリ解放だけではその値のdestructorを実行できない点も設計対象です。

request arenaの導入には、借用データの脱出、async中の寿命、DB workerへのmove、再試行と共有cacheへの保存を区別する必要があります。0.1では安全に所有化する場所を明示し、測ったallocationが本当に減るかを先に確かめます。
