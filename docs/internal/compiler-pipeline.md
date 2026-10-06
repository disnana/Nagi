# High / Low / Rustの内部経路

## 現在

実CLIの経路は`compiler/src/emit.rs::cli`で確認できる。

```text
High source → parse → module解決 → check
  → 型付きLow textと行対応表 → Low parse → 行位置を復元
  → 最終factoryでnative Low / @replaceを統合・check
  → CheckedProgram / 私有生成plan → Rust生成 → Cargo/rustc

Low source → parse → module解決 → 最終factory → CheckedProgram → Rust生成
```

Low textは保存用dumpだけではなく、通常High buildが通る内部境界でもある。`generated.low`はnative統合前のHigh部分であり、最終的に実行するプログラム全体ではない。

Phase 2のbuild/runでは、初回checkとinput保護の後、canonical outのOS lock内でこの生成経路を通す。stable app IDの新しいstagingでCargoを起動し、世代固有binのexeをコピーする。`BuildGeneration::finish(self, …)`がlock継承・公開・互換出力・latest置換の順序を固定する。runはlockを解放してから、そのbuildが選んだpathを起動する。High check/lowerの書込みも同じlockを使うが、非書込みeditor checkやLow checkは待たせない。

stagingと公開先は同じ親・深さに置き、相対runtime参照を保つ。世代に保存するsource/provenanceは実際に読んだtextと既存行対応であり、保存Lowから失われたHigh位置を再構築するものではない。可変の互換出力は直接Cargoや調査のために残す。失敗時に互換出力の一部が更新されても、以前の成功latestとexeは維持する。[ADR 007](adr/007-build-generations.md)に観測範囲と保証しない条件を記す。

nativeの通常関数はHighの初回名前解決・検査にも使う。`@replace`の本体を含む最終プログラムの統合・検査はLow再解析後に行う。

Phase 1の差分では、Rust生成の入力を`CheckedProgram`に限定した。factoryがASTを所有し、checked factsと生成用の決定を確定する。optionalな型を持つ共通Program全体をTyped IRへ変更したわけではない。opt-in SQL検査は封印されたcanonical ASTを読み、CLIのRust生成前に行う。

| 情報 | High→Low text→Low AST | Low check→Rust |
|---|---|---|
| 型 | 推論済みの注釈を出力。式ごとのchecked型や検査の証明は持ち越さず再検査 | checked型を生成に使い、公開型と私有storage型を分ける |
| ownership / lifetime | 証明をserializeしない。originとmove状態をcheckerが再構築 | 最終factoryが選択したbody・flow/use factsとstorage/cleanupの決定を封印する。Rustが最終borrow/dropを検査 |
| module identity | module/定義ID metadataと別名を保持 | 解決済みIDからRust名を生成。表記名だけで標準builtin扱いしない |
| source | 同じコンパイル中は生成Lowの行から元の文・定義の行へ復元 | 元位置とGenerated/User/Native/@replace/Syntheticの由来を分離。置換対象のdefinitionを保持し、同じLow行の複数関数も区別 |
| 保存・再読込 | 保存Lowを独立コマンドで読むと位置はLow。High source mapは保存しない | Rust primary spanに対応がある時だけNagiのファイル・行を先に示す |

`Program`はHigh/Low共通ASTだが、すべての状態で型が揃う専用Typed IRではない。`Expr.ty`などはoptionalで、名前解決とcheckerの状態にも依存する。「Common IR」と呼ぶだけではbackend前提の保証にならない。

[ADR 011](adr/011-language-behavior-and-docs.md)では、Lowを中間的な構文と呼ぶことと、安定したbackend非依存の意味論IRとして保証することを分けた。Nagiの観測可能な契約、標準APIのruntime契約、checked facts、Rust backendの表現は別の責任である。意味論を定義してRustへ実装を任せることは両立する。今回、この経路の撤去・全面IR化やself-hostingは行わない。

## 問題

1. textへの往復でcheckerの事実を失う。型付きLowとmetadataの出力漏れは、HighとLowの差になる。
2. 最終checkの後までemitterがbuiltin名・型の形・変数名からmove/borrowを再判定すると、checkerと二つの意味論を維持することになる。
3. 一つのRust placeに局所借用と返却する借用を繰り返し代入すると、Nagiが認めた短い寿命をRust側で長く結び付けてしまう。List専用の空値で退役させる方法はResult/Optionへ自然には拡張できない。
4. loopの初回検査のtyped bodyと、固定点まで統合した状態が混在すると、受理の根拠と生成の入力が違う。
5. 行だけのsource mapはRustの補足spanや式の列位置までは戻せない。位置を推測して埋めると誤診断になる。
6. scope本体を余分なasync blockへ入れると、本体の局所viewを元の外側cleanup anchorへ保存できない。通常のloop/asyncが通ってもScopeとの組合せでE0521/E0381になった。

1・5は現行の構造上の制約、2〜4・6は今回の生成設計の改善対象である。

## 選択肢

| 構成 | 利点 | 費用・注意点 |
|---|---|---|
| 現在のtext往復を維持 | 保存Low・手書きLow・@replaceが同じcheckerを通る。既存経路をそのまま検証できる | 再parse/check費用、metadata/位置のtransport、重複した検査 |
| High/Low parser→共通Typed IR→backend、Lowはprinter | 型・origin・module・位置を一度決定できる。検査済み境界が明確 | 統合と@replaceの時点、エディターAST、保存Lowの独立性、診断を移行する必要がある |
| 全CFG/SSAと独自borrow checkerへ変更 | 複雑な制御経路を統一して表せる可能性 | 既存Rustとの二重化が増える。現在の規模では根拠が足りず、修正費・移行費が大きい |

## 推奨

今回text境界を削除しない。最終Low checkの結果を使う生成境界から整理する。

- checkerの共通点で値の`Move / Copy / Borrow / BorrowMut`を決め、私有factsとして渡す。
- storage planは必要なview-containing bindingだけを扱う。名前へRustコードを埋め込まない。
- 各versionを元bindingのcleanup anchorで宣言し、通常Rustのmove/Option/Dropで破棄を表す。実行時の独自寿命管理やpayload cloneを追加しない。
- loopでは最終固定点のtyped body・条件・factsを使う。forのiteratorは一度、whileの条件は各反復と最後の終了判定で評価する。
- planの前提を検証し、facts欠落を新しいbuiltin推測で隠さない。
- scope本体は同じcoroutine内のlabelled Result blockへ出力する。`try`/nested scope/joinのErrは最寄りのscopeで子の取消を待ってから外側へ伝える。Error変換と元のbody-local cleanupを維持する。返却型やstorage planの有無で分けると、借用を返さず一時利用する関数にも同じasync境界の不具合が残るため、全scopeを共通化した。

これは全面Typed IRへの移行ではない。public Type、Low構文、@replace、Rust backend、runtimeを維持する。

## 次の移行を決める条件

最終codegen境界を封印する判断は[ADR 006](adr/006-sealed-codegen-input.md)で採用した。全面Typed IRは別の判断であり、High/Low/backendの差、parse/check時間、facts保持量を測って決める。封印だけをtext往復の費用・情報消失の解決とはしない。

移行するなら、(1) AST/check結果の境界を定義、(2) module/@replaceの統合を同じ境界に固定、(3) source identityを維持、(4)旧text経路と新経路を同じcorpusで比較、(5)保存・手書きLowを独立に維持、の順とする。互換性を確認する前に旧経路を消さない。

Lowを独立した低水準言語へ拡張する計画は当面止める。現在の実利は波括弧構文、生成内容の確認、関数差し替えであり、pointer/layout/unsafe/C ABI/SIMDの実装はない。これだけでRust直書きより優れると主張しない。
