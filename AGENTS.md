# Nagiリポジトリを開発するエージェントへ

ここはNagi compiler/runtime/editor/repository自体の作業指示。Nagiでアプリを書く指示は[ai/README.md](ai/README.md)にある。

## 判断の根拠

優先順: 明示した言語契約 → 承認済み設計判断 → `DESIGN.md` → 公開リファレンス → conformance/regression tests → 現在の実装。未採用の設計案を承認済み契約にしない。実装やテストが契約に反する場合、現状に合わせて期待値を緩めず、矛盾を報告する。

契約は[language-invariants.md](docs/internal/language-invariants.md)、内部経路は[compiler-pipeline.md](docs/internal/compiler-pipeline.md)、テスト分類・実行法は[compiler-testing.md](docs/internal/compiler-testing.md)。新しい依頼の明示的な条件はこれらより優先する。

コンパイラ/Rust境界の段階作業は[計画書](docs/internal/compiler-rust-boundary-plan.md)に従う。Guarantee Registerの現在と予定を区別し、前PhaseのacceptanceとCI成功前に次の実装へ進まない。[未決事項](docs/internal/open-questions.md)がStop扱いなら、契約・test期待を先に変更しない。結果と未確認範囲は[進捗](docs/internal/progress.md)へ記録する。

Stop対象のtest期待は、公開言語意味論・CLI/API利用者契約・High/Low互換性・Guarantee Register・security/lifecycleの保証に関わるもの。承認済み設計に伴う内部生成先・file名・path等は、変更理由と維持する保証を記録して更新できる。内部assertの更新を口実に公開保証や失敗の観測を弱めない。

## sub-agent / model 運用

sub-agentは担当を独立して切り出せる場合だけ起動する。親agentが数行で安全に完了できる変更、短いDocs修正、同じfileを同時編集する作業、単なる念のための重複調査には起動しない。探索、独立した検証、実装と最終reviewなど、成果物と責任範囲が明確な場合に限る。起動時は対象、期待成果、編集範囲、必要な検証を指定する。子agentからの追加spawnは禁止する。

| role | model ID / reasoning effort | 用途 |
|---|---|---|
| Light Worker | `gpt-6-luna` / `medium` | 範囲が狭く、contract判断を含まないDocs、機械的rename、読み取り調査。曖昧な仕様や重要なcode correctnessが出たら親へ事実を返す |
| Fast / Worker | `gpt-6-luna` / `max` | 仕様が明確なroutine実装、feature/test fixture、明確なCI修正、単純bug fix、小規模refactor。既存契約に沿い、対象に必要な検証を最後まで行う |
| Deep / Engineer | `gpt-6.1-sol` / `high` | 重要・難度の高いcode、複数moduleやcompiler/runtime境界、原因不明bug、async/concurrency/ownership/lifecycle、API contract、重要なDB/storage/security変更 |
| Independent Reviewer | `gpt-6.1-sol` / `high` | 実装・必要な実行検証後の独立した最終audit。contract、回帰、ownership/lifecycle、error、concurrency、security、compatibility、test evidenceを確認し、編集しない |
| Architect | `gpt-6.1-sol` / `xhigh` | Highで決められない言語意味論・architecture・compatibility、subtle correctness、race/lifecycle設計。未解決の判断を絞り、案と保証を示す |
| Critical Reviewer | `gpt-6.1-sol` / `max` | High/xHighで未解決の難問、release直前または誤判断コストが極めて高い変更の独立最終review。通常開発には使わない |
| Astra Final Verifier | `gpt-6-astra` / `max` | Sol Maxでも解けない問題、または誤判断コストが極めて高い独立最終検証。日常開発・通常reviewでは使わない |

taskの種類ごとに、正しさと必要なaudit証拠を満たす完了までの見込み費用が最小のroleを選ぶ。contractが明確な実装/fixtureはFast / Worker、重要・難度の高い実装はSol High、狭いDocs/rename/read-onlyはLight Workerを使う。数行の作業は親が行う。実際のmodel時間、test/CI実行時間、queue/wait、手戻りは測れた場合に別々に記録し、体感を測定値として扱わない。下位roleの失敗や見込み時間・品質・総費用の改善が見られないなら、同じ試行を繰り返さず具体的根拠で方針を見直す。

昇格は `Luna Medium → Luna Max → Sol High → Sol xHigh → Sol Max → Astra` を目安とするが、順番に試す必要はない。High/xHighでないと扱えない課題は初手からそのroleに割り当てる。具体的に未解決のcontract、再現、diagnostic、ownership、race境界が見つかったとき、または同じ原因の対象試行が2回続けて失敗したときに止め、証拠と未解決点を適切なroleへ渡す。既に得た調査を次のroleに渡し、同じ探索や全suiteを繰り返さない。失敗原因をAI modelだけに帰属させず、実際に通したstage、未実施の検査、fixture/sourceの不備を分けて記録する。

同時に進行するagentは親を含め最大3体とする。Codexの`agents.max_concurrent_threads_per_session`はspawnした子threadを数えるため2に設定する。同一model IDとreasoning effortの組合せは同時に1体とし、Deep EngineerとIndependent ReviewerはともにSol Highなので順番に実行する。重要なcodeの独立reviewは必要な実装検証後に行う。親+2子は互いに依存しない成果物が明確な場合だけ使う。Sol Maxも同じeffortのagentを並行させず、Astraはeffortに関係なく全体で1体までとする。同じ課題を複数agentに競わせない。

認証/認可、秘密、整合性、concurrency、破壊的変更では、実装前にSol 6.1 Highで承認済みdesignと必須failure/regression casesを確認する。未決の意味論・代替案・race/lifecycle設計がある場合だけSol 6.1 xHigh Architectへ上げる。実装・必要検証後は別のSol 6.1 High Independent Reviewerが最終auditし、最終diffを仕様・実行結果へ直接照合して関連regressionを確認する。指摘修正後は対象testを再実行し、更新diffと結果を再reviewしてから完了する。その他のsecurity/public contract/migration/ownership/lifecycle/correctness重要変更も、実装者の自己reviewだけで完了にせずIndependent Reviewerを使う。source-only reviewをparse/check/build/native/CIの代わりとして扱わない。Sol Highを使う実装者や親agentの同tier作業が終わるまでHigh Reviewerを並行起動しない。

`Fast Worker`はrole名で、pricing画面のservice tier `Fast`とは別である。`.codex/agents/*.toml`はmodel IDとreasoning effortだけを指定し、現在のagent実行interfaceにもservice-tier/speed指定がない。未対応の`service_tier` fieldを追加しない。既存の明示承認は`gpt-6-luna` / `max`のFast tierに限り、runtimeが明示的に提供するとき利用できる。他modelのFast、追加購入/pay-as-you-go/plan/add-on、または現在の承認を越える課金を伴う変更は事前確認する。通常taskに必要な範囲のmodel/effort選択やSolへの昇格は、追加購入を発生させない限り確認待ちにしない。表示価格のFast倍率が2倍でも速度が2倍とは推定しない。`max` reasoning effortもservice tierや速度を表さない。

`.codex/agents/*.toml`はCodexがrole fileとして自動検出し、別のrole登録表は不要。各roleにtop-level `name`、`description`、`model`、`model_reasoning_effort`、`developer_instructions`を設定する。`.codex/config.toml`は子thread上限を設定する。起動前に実行環境のmodel catalogとeffortを確認し、未対応または未確認のIDは使わず、別IDへ暗黙fallbackしない。

設定仕様、価格/benchmark evidenceと未確認範囲は[agent構成の記録](docs/internal/agent-routing.md)に残す。

2026年10月11日時点のrouting判断は、ユーザー提供のcompiled report（2026-10-11版は「Business/Enterprise」quota creditsを主張、発行元/primary URL未確認。2026-10-08版はArena値を主張）、このsessionのagent tool catalog、および2026-10-06にpinしたCodex role/config schemaを参照する。API USD料金、Codex quota credits、Fast倍率、実測速度は別指標として扱い、未確認値は未確認のまま残す。reportに記載された値とmodel別の限界は[agent構成の記録](docs/internal/agent-routing.md)を参照する。

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
| compiler behavior / conformance fixture | High/Lowの入力parse、positive/negative check、negative reasonとprimary source line、生成Rust buildを別々に確認する。emission/backend経路に関わる変更は最小native例まで実行する。source-only reviewはこれらの代わりにならない。必要な検証が実行できない場合は未確認として残し、exact-head CI前にReadyと報告しない |
| resource metadata / capability | registryの独立した手書き期待と集合完全性、generic roleのindex範囲・重複・欠落、operation Passing/borrow_owner、field accessorを確認。用途別遍歴とnative Debug/Chargeを同一solverへ潰さない。未正規化High/保存Low一致、stable logical IDの全文golden、実Cargo/native・negative元位置を維持。新targetの4 OS実行を確認し、harness登録だけを実行成功と数えない。Phase 3はtest-only CI成功後に集約する |
| storage / lowering | 値・評価順、RHS Err/panic、旧値Drop panic、正常/Err/unwindの破棄位置。asyncなら未poll・pending・再開・取消、scopeならjoin/兄弟取消。allocation/clone/Futureサイズを測り条件を報告 |
| parser / diagnostics | 不正・上限入力、High/Low、文字/byte/UTF-16位置、元module位置。対応のないRust spanを推測変換しない |
| HTTP | success、handler Result、handler/mapper panic、timeout、malformed/過大本文、shutdownとcapacity解放。実socketテストを使う。応答開始後やnon-yielding処理の制限を残す |
| DB / SQL | opt-in/staticとruntimeを分ける。列名/必要返却列/bind、動的SQL、NULL/値型、schema不一致、worker終了。transaction/cancellationの保証を勝手に追加しない |
| actor / supervisor | admission、reply Errとworker Err/panicの違い、timeout、再起動条件、最後のworker、所有者Drop/shutdown、context/task/permit解放 |
| build / publish / cache | 既定・明示cache、app ID同一/相違、同じoutのOS lockと待機barrier、旧exeのbytesと実行、親終了後のCargo、公開後のmanifest参照、input/output identity保護、Cargo/投影/latest置換失敗。run前のlock解放と失敗時の旧latest保持を確認。常設lockをunlinkしない |
| 文書のみ | リンク、日英、実装・版・サンプルとの整合。Rust全suiteは文書だけの変更では通常不要 |

対応する既存harnessを使い、同じassertだけのテストを増やさない。拒否をacceptへ変える、assert削除、seed除外、失敗をskipへ変えることで検査を通さない。設計上必要な期待変更は理由とbefore/afterを示す。

並列testの一時directoryは、PIDと時刻だけで一意と判断しない。同じclock tickでも別の所有者へ分かれ、exclusive作成に成功したdirectoryだけをDropで削除する。fixtureの衝突をglobal test直列化や成功までの再実行で隠さず、同tickの回帰で確認する。

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
