# Nagiリポジトリを開発するエージェントへ

ここはNagi compiler/runtime/editor/repository自体の作業指示。Nagiでアプリを書く指示は[ai/README.md](ai/README.md)にある。

## 判断の根拠

優先順: 明示した言語契約 → 承認済み設計判断 → `DESIGN.md` → 公開リファレンス → conformance/regression tests → 現在の実装。未採用の設計案を承認済み契約にしない。実装やテストが契約に反する場合、現状に合わせて期待値を緩めず、矛盾を報告する。

契約は[language-invariants.md](docs/internal/language-invariants.md)、内部経路は[compiler-pipeline.md](docs/internal/compiler-pipeline.md)、テスト分類・実行法は[compiler-testing.md](docs/internal/compiler-testing.md)。新しい依頼の明示的な条件はこれらより優先する。

## 作業手順

1. 対象の契約、関連実装、pass/failテスト、変更履歴を読む。parse成功、check成功、Rust build成功、実行成功を区別する。
2. 不具合は小さい再現を作り、失敗段階と診断を記録する。似た例が2件以上なら共通するorigin・use・control flowの欠陥を調べてから直す。
3. 元ソース/checker/loweringを修正する。生成Rust、`generated.low`、ビルド成果物を直接修正しない。第三者のcrateやコピーしたbackendコードを場当たり的に変更しない。
4. 意味論の例、実装、必要な検査、文書を揃える。未対応を黙って成功にしない。unexpected backend rejection、ICE、欠落したchecked factsは不具合として扱う。
5. 最終差分を再確認する。PRは通常最新mainをbaseにする。stacked PRなら依存と最終反映先を明記し、feature branchへのマージをmain反映と報告しない。main反映は実際のcommit/treeを読み戻して確認する。

互換性を保つ内部修正は進める。syntax破壊、public semanticsの大幅変更、既存API削除、Low廃止、backend/runtime全面交換は根拠・移行案を示す。セッションで既に許可された作業を、文書だけを理由に再承認待ちへしない。

## 必須の境界

- サポート範囲のNagiをcheckerが受理した後、生成RustがNagiで検出可能な型・move・lifetime問題で拒否されるのはP1。rustcへの委譲は手書きRust、crate API/traits、最終Send/Sync/Clone、link/target/依存環境など。委譲を生成ミスの説明に使わない。
- HighとLowの型・所有権意味論を揃える。通常High CLIは生成Lowを再parse/checkする。保存LowとHigh source mapの寿命を混同しない。
- move/borrowの意味をemitterの名前リストで再実装しない。checked factsと私有生成planを使う。公開型、source origin、binding ID、synthetic slot IDを分ける。
- capability検査では関数署名・phantom markerと実payloadを区別する。標準API内部のArc state/contextも共有境界である。
- clone、allocation、Drop、評価順、Future frame、取消への影響を確認する。RustのDrop/borrow/Futureを使い、独自runtimeの寿命管理を安易に追加しない。
- Resultの業務Err、panic、取消、compile error、infra errorを区別する。panic捕捉はrollbackではない。取消は受理済みDB操作や外部副作用を戻さない。
- 生成先とCargoキャッシュは別の境界。同じtarget内の別アプリの実行ファイルを上書きしない。ビルド経路の変更では既定・明示キャッシュの両方を検査し、テストだけのcwd隔離で利用者の競合を隠さない。

## 変更ごとの検査

| 変更 | 必要な観測 |
|---|---|
| ownership / lifetime / 型 | positiveとnegative、拒否段階と元位置、High check、保存Low check、関係する手書きLow、生成Rust build。分岐・loop・function value・nested Resultが関係するなら組合せも確認 |
| storage / lowering | 値・評価順、RHS Err/panic、旧値Drop panic、正常/Err/unwindの破棄位置。asyncなら未poll・pending・再開・取消、scopeならjoin/兄弟取消。allocation/clone/Futureサイズを測り条件を報告 |
| parser / diagnostics | 不正・上限入力、High/Low、文字/byte/UTF-16位置、元module位置。対応のないRust spanを推測変換しない |
| HTTP | success、handler Result、handler/mapper panic、timeout、malformed/過大本文、shutdownとcapacity解放。実socketテストを使う。応答開始後やnon-yielding処理の制限を残す |
| DB / SQL | opt-in/staticとruntimeを分ける。列名/必要返却列/bind、動的SQL、NULL/値型、schema不一致、worker終了。transaction/cancellationの保証を勝手に追加しない |
| actor / supervisor | admission、reply Errとworker Err/panicの違い、timeout、再起動条件、最後のworker、所有者Drop/shutdown、context/task/permit解放 |
| 文書のみ | リンク、日英、実装・版・サンプルとの整合。Rust全suiteは文書だけの変更では通常不要 |

対応する既存harnessを使い、同じassertだけのテストを増やさない。拒否をacceptへ変える、assert削除、seed除外、失敗をskipへ変えることで検査を通さない。設計上必要な期待変更は理由とbefore/afterを示す。

## 実行コマンド

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked -p nagic --test conformance
cargo test --locked -p nagic --test view_flow_completion --test view_container_drop
cargo test --locked -p nagic --test scope_runtime_contract --test sql_check
cargo test --locked -p nagic --test constant_validation --test auth_boundaries
cargo run --locked -p nagic --example fuzz-smoke
python scripts/verify_application_examples.py
```

対象の検査から始め、処理系/runtime変更の最終確認では`cargo test --locked`も実行する。HTTPとscope統合にはsocket、Rust/Cargoとruntime依存が必要。コマンド失敗をネットワーク・toolchainなどのinfra失敗と契約違反に分ける。定期生成・失敗artifactの設定はテスト文書を参照。

## 文書と完了条件

人向けDocsを内部監査ログで埋めない。内部契約・検証は`docs/internal/`、アプリAI向けは`ai/`へ置く。public APIや制約を変えるなら公開Docsと英語版も更新する。未リリースの修正を公開済み版の機能と書かない。版を上げる作業は他の修正と区別する。

完了報告には変更、検査対象と結果、未確認target、保証の限界を残す。行数・テスト件数・AIの成功報告を品質の証明にしない。残す問題はseverity・理由・必要な判断・次の行動を記録する。TODOだけで完了にしない。commit/PR/commentは日本語を使う。
