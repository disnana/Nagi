# Phase 1: 封印された生成入力の検証

2026-10-05。基点は#77を反映したmain `ded4c44cd3ebf984b382995322cefc769b4a6cb3`。以下は開発branchの結果であり、公開版の保証ではない。4 OS・JetBrains CIは未確認。

## 原因と変更

旧`emit::rust(&Program)`は、parseしただけのASTも受け取れた。型・名前・引数の渡し方などのchecked factsと、emitterが作り直す生成判断が分かれていた。

[最終factory](../../compiler/src/check/checked.rs)がLow/native統合、`@replace`、check、必須factsの検査、生成planの確定を行い、成功時だけ`CheckedProgram`を返す。Programはmoveし、公開APIからは共有参照でのみ閲覧できる。[emitter](../../compiler/src/emit.rs)はこの入力だけを受け取る。

Copy・Serde等の判定、view storage・reborrow・cleanup・Rust型表記は封印時に確定する。既存の判定順を移し、言語の受理条件やDropを変える作業とは分けた。canonical ASTとRust名に変換した私有ASTを保持し、Rust textそのものをchecked representationとして保存しない。採否と代替案は[ADR 006](adr/006-sealed-codegen-input.md)。

## 検証で見つかった穴

いずれもPhase 1実装中の封印・provenanceの欠陥。既存プログラムの仕様を変更して解消したものではない。

| 分類 | 観測 | 対応 |
|---|---|---|
| P1 | 宣言型・binding型・pattern型が欠けても封印できた | 文・bindingを含む共通facts検査を追加。故障注入は再checkで情報を補わず拒否を確認 |
| P1 | 必要なview flow planやstorage slotを消しても生成へ進めた | 封印時の必要slot集合を保持し、生成前に欠落を検知 |
| P1 | enum/標準constantのownerを未型付けpayloadと誤認した | symbolic ownerの解決と値expressionを区別。既存の受理を維持 |
| P2 | 同じnativeソース行の複数関数・置換本体の由来を区別できなかった | 関数のlineとcanonical symbolによるlineageを保持。元位置とは分ける |

P0を発見したとは報告しない。この検査を任意の内部AST改変に対する完全性証明とは扱わない。

## 検証結果

- `cargo fmt --all -- --check`、`cargo clippy --locked --all-targets -- -D warnings`。
- `cargo test --locked`: 89 suite、784成功、失敗・ignoreなし。新しい封印境界unit testは15、API compile-failは3。既存ownership、Result、SQL、HTTP panic、Drop、scope取消、実Cargo/native診断もこのsuiteに含む。
- SQL engineなしのbuildとCLI/SQL test: 8成功。
- 拡大conformance: seed `305419896`と`3735928559`で各256生成caseと38固定corpusをHigh/保存Lowからnative実行まで検査。各runのharness testは5成功。
- mutation smoke: 10,000入力、parse拒否7,149、check拒否1,910、check後Low/生成まで941、panic 0。別に128 bounded native caseを検査。coverage-guided fuzzではない。
- VS Code: 196成功。HTML viewport: 5成功。conformance登録: corpus 38、linked harness 11。
- 旧版とのbyte比較: 17正例についてLow、直接生成Rust、保存Low生成Rustの51ファイルが一致。全プログラム、外部crate、全runtime構成の同値証明ではない。

最初の全suite実行はsandboxの通信制限により、HTTPのloopback bindが35件失敗した。通信を許可して同じsuiteを再実行し成功した。最初の失敗を成功やignoreに置き換えていない。VS Codeの初回実行もcompilerをPATHに置いていないため失敗し、同じテストを今回buildしたcompilerで再実行した。

frontendの時間・process RSS・生成内容は[測定記録](../../benchmarks/results/frontend-sealing-2026-10-05/README.md)を参照。共有host上のdebug buildによる小さな比較であり、runtimeの性能向上は主張しない。

## 保証と限界

封印API、既知の必須facts欠落、同入力の決定性、登録したHigh/Low/nativeの契約を継続検証できる。初回High check・エディターの回復ASTからRust生成へ直接渡すAPIはなくなる。

生成由来はGenerated Low・User Low・Native Low・置換本体・合成glue・Unknownに分ける。最終checkで自動生成側の拒否が起きた場合はcompiler defect候補とし、native混在や由来不明の場合は未分類に残す。rustc診断に由来を添えても、そのエラーの原因まで自動確定したとは扱わない。

source mappingは既存の文・定義行単位。式column、全Rust診断、任意crate内部の位置変換は保証しない。手書きRustのエラーはRust位置を保持する。

CheckedProgramはNagi checkerの受理状態であり、Rust本体・crate API・traits・Send/Sync・最終borrow安全性・link・依存環境の成功証明でもsandboxでもない。runtimeの意味論、暗黙clone、allocation、Future frameを変更する最適化は行っていない。

## 残課題

| 分類 | 課題 | 次の行動 |
|---|---|---|
| P1の観測・予防 | サポート範囲の未知check/build mismatchを排除したとは言えない | 同じcorpus/生成/mutationをCIで継続。新反例はstageと元sourceを保存して縮小 |
| P2 | ASTにoptional factsが残り、最終factoryで欠落を検査する | 今回は封印境界まで。全面Typed IRやLow text撤去は別判断 |
| P2 | capabilities/resource情報の重複 | Phase 3で現在のcharacterizationを先に固定してから共通化 |

## CIの選択

最初のPR #78のCIでは、compiler変更でLinux/JetBrainsは起動したが、4 OSの`nagi-package`はskipされた。既存release planが版更新と配布設定の変更だけを配布検証の条件にしていたためで、4 OS成功とは数えない。

compiler/runtime/Cargo入力の変更もNagiの配布検証を起動するよう、release planへ条件と回帰テストを追加する。公開の条件は引き続き版更新であり、検証用packagingを正式releaseとして公開しない。Docs-onlyではこの条件を使わない。

Phase 2は、このPhaseのacceptanceとCI成功を確認した後に着手する。mainへのmerge、版更新、releaseは行わない。
