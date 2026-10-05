# ADR 002: 型付き定数検査

状態: 採用。既存のprofile依存overflowを保つ。

## 共通モデル

型・名前解決後の副作用なし整数/boolean式を、一つのvalidation moduleで検査する。8固定整数幅を保持し、Known、Unknown(reason)、Errorを区別する。未型付け/壊れた内部状態をUnknownとして隠さない。生成側はfoldや定数評価を再実装しない。

除数0、signed MIN/-1とMIN%-1はprofile非依存の静的失敗。正常な商は0方向へ切り捨て、剰余は左辺符号に従う。+/-/*とMINの単項否定のoverflowは既存どおりdebug panic/release wrapなのでProfileDependentとしてKnownへ折り畳まない。

小さいscalar factsで純initializer・明示再代入を追う。分岐は同じKnownの継続枝だけを保持し、loop書込は保守的にkillする。関数評価、Rust本体、field/index、target依存size_of、代数的なx-x簡約は行わない。名前が同じだけのユーザー関数をbuiltinとして評価しない。

## 互換性と代替案

literal0は現行でも到達不能な式で拒否される。今回のcompound/alias zeroとMIN除算も同じ静的禁止として構文全体に適用する。従来受理されたdead compound式が拒否される診断拡張であり、互換性への影響を公開Docsに記す。

代替はrustcの到達可能性/定数伝播をコピーする方法だが、Rust版依存の受理集合と二重のcontrol-flow意味論を作るため採らない。条件のKnownをownership受理緩和やcodegenの枝削除に使わない。既存の+/-/*overflow許可を静的拒否へ変える案も採らない。

## 診断

型・名前エラーを優先し、正常なtyped ASTだけを新passへ渡す。既存literal診断の順序を維持し、追加診断はsource順に発生させる。nested失敗を親の演算より先に示す。除数0は除数位置、MIN/-1は演算位置。元module行、High/Low、安定したcode・primary/関連位置を検査する。

二重MIN否定のRust literal拒否はprofile依存で免責せず、typed MIN leafの正しい印字で修正する。式全体をfoldせず、debug panic/release wrapを実行して確かめる。
