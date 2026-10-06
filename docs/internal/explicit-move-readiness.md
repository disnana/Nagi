# PR #87: moveの仕上げ監査

2026-10-06。対象は[PR #87](https://github.com/disnana/Nagi/pull/87)。初回公開head `e4089735de35e5f8496a1191b870e7b5f7edffc9`の実装と、補強commit `44c38b7687d2b95830f673f7d57c29745dc9e567`を区別して確認した。mainは`7d2d96a8bdba191e456f796bdf477b95bc34c57e`。merge・release・版更新は今回の対象外。

## 状態と判定条件

初回headはdraft・mergeableで、[checks](https://github.com/disnana/Nagi/actions/runs/37447928596)・[website](https://github.com/disnana/Nagi/actions/runs/37447928084)が成功。4 OSの各原ログでmove9件・conformance6件・installation4件の実行を確認した。レビュー提出・未解決thread・通常コメントは監査時点で各0件。外部レビューの承認があるという意味ではない。

補強後のローカル検証は成功した。最新公開headのCIはPRのChecksから確認し、古いheadの成功を新しいheadのCI成功と数えない。draft解除の条件は、以下の検証、最新headの必須CI、未解決レビューの再確認を揃えること。Ready to mergeというjob名もmergeの許可ではない。

## 今回の補強

Sol 2人が実装・公開Docsを独立に読み、rootもchecked factsとRust生成を照合した。未解決のP0/P1、導入したTODO、狭い代入移行の外への義務拡大は見つからなかった。これを全プログラムの正しさの証明にはしない。

- `docs/ownership.md`と英語版へ、moveの必要性、元変数の再初期化、Copy入力はmove後も使えることを補足。
- 入門日英へ、元を残すにはmove後にcopyを追加せず、代入を`copy(view(name))`へ置き換えると明記。
- [既存9群](../../compiler/tests/explicit_moves.rs)の正常native群へ、Copy元の再利用、明示操作なしの非Copy引数・return、match payloadの裸returnを追加。元のpointer・Drop・borrow・取消assertは維持。
- compiler本体/runtime/依存/CIのbytesは初回headから変更していない。テストは上記の正常oracleを追加した。Rust標準identityの単一by-value評価と[OWN-04](language-invariants.md#own-04-明示moveの確定仕様実装済み未リリース)の範囲を保つ。

## moveの説明の照合

| 説明 | 実装・Docsとの対応 |
|---|---|
| 何か | 値と後片付けの責任を渡す。操作自体はclose・rollbackではない |
| なぜ必要か | 非Copy既存ローカルの代入で値を手放す意図を示す。コンパイラがcopy/sharedを代わりに選ばない |
| move後の変数 | 非Copy元の値を読む・再moveすることは拒否。新しい値で再初期化すれば名前を再利用できる |
| Copyとの差 | Copy通常代入は元も使える。moveを指定しても元は使える。判定表・新値生成は据置 |
| view/sharedとの差 | viewは読み取り借用、sharedは同じ値の共有。clone_sharedは所有handleを増やし、moveは増やさない。独立した値はcopy |
| 移行範囲 | 所有する非Copy localそのものの代入だけ。引数・return・field/index・try・matchの既存consumeは維持 |

公開説明は[所有権](../ownership.md#代入と明示move)と[英語版](../en/ownership.md#assignment-and-explicit-move)。仕様はDESIGN日英・[ADR 011](adr/011-language-behavior-and-docs.md#own-04-既存所有値の代入)・OWN-04に同期済み。今回の追記は説明と観測の補強で、新しいmove意味論の追加ではない。

## ローカル検証

`44c38b7`（tree `155784cfc0b98cc8a35338553473c545e50de187`）で`cargo test --locked`が成功。上位test binary/doctestの最終summaryを数え、90 suite・899件、failed/ignored 0。原ログSHA-256は`a753c6b8f063b49ad7f0c6064d0e3ffcd3e5ceba23ba0c4272c2cefbd3481348`。move専用9群と3-source native計9回を含む。テスト関数数を増やさず、今回の正常oracleを補強した。

fmt、全target clippy、日英7箇所・4種類の完全例check/run、website90ページとlocal links/anchors/assetsの検査が成功。先行確認では手書きLowの追加fixtureをRustのmatch文法で誤記してparserに拒否され、Lowのcase構文へ修正した。失敗をcompilerの成功や新しいバグと数えていない。

今回の[実行記録](../../benchmarks/results/explicit-move-2026-10-06/readiness/verification.json)、[全回帰原ログ](../../benchmarks/results/explicit-move-2026-10-06/readiness/full.log)、[実装Solレビュー](../../benchmarks/results/explicit-move-2026-10-06/readiness/implementation-sol-review.md)、[Docs Solレビュー](../../benchmarks/results/explicit-move-2026-10-06/readiness/docs-sol-review.md)を保存した。レビュー担当が独立にテストを再実行した記録ではない。

以前の93 suite・902件はgraph_render内の子process再実行3件を重複計上していた。旧全回帰も上位の90 suite・899件であり、原ログと期待は変更しない。stdout interleaveで子summaryが混ざる場合があるため、全summaryの単純合計を避ける。[集計規則](compiler-testing.md#oracleと段階境界)に記録した。

## 残る範囲と次工程

任意programのcheck/build一致、全allocation・Future配置、Rust extern/traits/依存環境はこの有限検査で保証しない。check時間の増加と共有環境・実行順の限界は[測定記録](../../benchmarks/results/explicit-move-2026-10-06/README.md)に残る。速度向上や普遍的なzero-costを主張しない。

前回の確認二点はS1の詳細設計で、#87のmoveの残件ではない。今回の委任による初版選択は全Tの正常await/discardと、受取後も残るscope故障。旧statement spawnとSupervisor terminal→HTTP取消を保ち、新Taskのparse/check/runtimeは別工程で作る。次の実装前にS1→業務Err/faultのS2→公開Pool/Txの依存を確認し、spec→先行test→私有bridge→公開配線→conformance/4 OSの順を維持する。

SQLiteの[巨大capacity](sqlite-capacity-decision.md)は公開化前のP2設計ブロッカーとして残る。新Taskのjoin記録、typed receiver、取消再開、actual join後の受取、メモリ/Drop/terminal連携は未実証。この監査はそれらを完成済みとは扱わない。
