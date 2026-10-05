# ADR 004: checkを分ける境界と移行順

状態: 責任分担を採用。専用HIR/CheckedProgramへの全面移行は保留。

## 現在

共通Program ASTにoptionalな式型と名前解決情報を記録する。通常checkは最初のエラーで停止する。エディターの回復用ASTは別経路で、codegenの入力に使わない。PR #74のmove/borrow/use factsとstorage planを維持する。

今回の定数検査は、名前・型・ownership検査が成功した後のtyped ASTだけを読む。既存literal-zeroの早期診断も同じconstant moduleに置く。domain検査から型推論・所有権判定を再実装しない。

## 責任分担

| 段階 | 受け渡すもの | 成功が意味しないもの |
|---|---|---|
| parse | 構文ASTとfile-local token span | 名前・型・所有権の正しさ |
| module/name resolution | canonical module/definition ID、標準定義のidentity | Rust本体やcrate APIの存在確認 |
| type/ownership/control flow | 型、origin、move/use、最終loop/branch facts | 全Rust trait条件・任意Rustの安全性 |
| domain validation | 型付き整数の静的失敗、明示schemaのSQL、宣言されたproof型の受渡し | 全SQL・全routeの認可・実DBの状態 |
| codegen plan | checked facts、cleanup anchor、synthetic slot | Rustで拒否されたNagi生成ミスの免責 |
| Cargo/rustc | 実adapter/crate/trait/targetの整合とnative生成 | panic後の状態回復、policyの業務上の正しさ |

この表は責任の整理であり、現在すべてが別のpass/moduleになっているという説明ではない。型・ownership/control flowはまだ大きいchecker内にある。proofの受渡しには既存型検査を使い、新しい一般effect/taint解析は追加しない。

## エラー回復

現行fail-fast checkに、publicなErrorType/UnknownTypeを追加して推論を続ける必要はない。専用HIRを導入する場合は、(1) valid type、(2)先行診断に紐付いたerror、(3)未解決、を区別し、後二者からcodegenできない境界を先に作る。constant評価のUnknownは動的・profile依存の値であり、frontendの型エラーとは異なる。

## 保留した変更

High/Low text往復の撤去、全CFG/SSA、独自Rust borrow checker、任意crateの型自動import、一般async callback、式spanの完全transportは今回実装しない。現在のconformanceで解決できない具体例と費用測定が得られた場合に、[compiler-pipeline](../compiler-pipeline.md)の段階移行を使う。新passのためだけに#74のfactsを捨てない。

## 可視化

認可フローの自動表示は後続候補。Graph IRのcanonical type/function/source IDを使い、Principal・Grantの明示署名と登録されたpolicy境界を別の関係として表示する。名前やSQLから認可を推測しない。Rust本体内の呼出し・実行時policy条件は不明と表示する。初回のデモ図は手書きで、自動生成やセキュリティ証明とは区別する。
