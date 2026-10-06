# Nagiリポジトリを開発するエージェントへ

ここはNagi compiler/runtime/editor/repository自体の作業指示。Nagiでアプリを書く指示は[ai/README.md](ai/README.md)にある。

## 判断の根拠

優先順: 明示した言語契約 → 承認済み設計判断 → `DESIGN.md` → 公開リファレンス → conformance/regression tests → 現在の実装。未採用の設計案を承認済み契約にしない。実装やテストが契約に反する場合、現状に合わせて期待値を緩めず、矛盾を報告する。

契約は[language-invariants.md](docs/internal/language-invariants.md)、内部経路は[compiler-pipeline.md](docs/internal/compiler-pipeline.md)、テスト分類・実行法は[compiler-testing.md](docs/internal/compiler-testing.md)。新しい依頼の明示的な条件はこれらより優先する。

コンパイラ/Rust境界の段階作業は[計画書](docs/internal/compiler-rust-boundary-plan.md)に従う。Guarantee Registerの現在と予定を区別し、前PhaseのacceptanceとCI成功前に次の実装へ進まない。[未決事項](docs/internal/open-questions.md)がStop扱いなら、契約・test期待を先に変更しない。結果と未確認範囲は[進捗](docs/internal/progress.md)へ記録する。

Stop対象のtest期待は、公開言語意味論・CLI/API利用者契約・High/Low互換性・Guarantee Register・security/lifecycleの保証に関わるもの。承認済み設計に伴う内部生成先・file名・path等は、変更理由と維持する保証を記録して更新できる。内部assertの更新を口実に公開保証や失敗の観測を弱めない。

## sub-agent / model 運用

sub-agentは担当を独立して切り出せるときだけ起動する。通常はまずFast / Workerで処理できるか判断する。短い修正、親agentがすぐ終えられる作業、同じファイルを同時編集する作業、単なる念のための重複調査には起動しない。探索、互いに依存しない検証、実装と独立reviewなど、成果物と責任範囲が明確な場合に限る。起動時は対象、期待成果、編集範囲、必要な検証を指定する。sub-agentにさらにsub-agentを起動させない。

| role | model ID / reasoning | 用途 |
|---|---|---|
| Fast / Worker | `gpt-6-luna` / `max` | 仕様が明確な通常実装、既存パターンの機能追加、単純bug fix、テスト・fixture・Docs、rename/cleanup、明確なCI修正、小規模refactor。まずこのroleで十分か判断する |
| Deep / Engineer | `gpt-6.1-sol` / `high` | 複数module、compiler/runtime境界、原因不明bug、non-trivial refactor、async/concurrency/ownership/lifecycle、API contract、重要なDB/storage/security変更、Fastで詰まった問題 |
| Independent Reviewer | `gpt-6.1-sol` / `high` | 重大な変更を実装者から分離してreviewする。contract、regression、lifecycle、error handling、concurrency、security、backward compatibility、test coverageを確認する。通常の独立reviewはこちらを使い、変更は行わない |
| Architect | `gpt-6.1-sol` / `xhigh` | 言語仕様・意味論、architecture/backward compatibility設計、案の比較、subtle correctness、race/lifecycle/resource ownership、release前の重要設計review。明確な理由を親が記録して起動する |
| Critical Reviewer | `gpt-6.1-sol` / `max` | release直前、High/xHighで未解決の難問、migration失敗の最終解析、誤判断コストが極めて高いsecurity/correctness変更。常用せず、独立review専用とする |
| Astra Final Verifier | `gpt-6-astra` / `max` | Sol Maxで解決できない最高難度、または誤判断コストが極めて高い独立最終検証だけに使う。日常開発・通常reviewには使わない |

昇格は原則 `Luna Max → Sol High → Sol xHigh → Sol Max → Astra`。初手からHigh/xHighでなければ扱えない言語意味論・architectureの課題は、その根拠を示して開始してよい。各段階の成果、根拠、未解決点を次のroleに渡し、調査を繰り返させない。High/xHighの通常開発で解けた課題をMaxやAstraへ送らない。

同時に進行するagentは親agentを含めて最大3体とする。Codexの`agents.max_concurrent_threads_per_session`はspawnされた子thread数を数えるため2に設定し、親1体と子2体までにする。同一model IDとreasoning effortの組合せは、同時に推論・作業するagent全体で最大1体とする。親が作業を子へ渡して待つ間は、同じ問題を並行推論しない。通常は親+子1体で足りると考え、親+子2体は明確に独立した仕事がある場合だけ使う。

Deep EngineerとIndependent ReviewerはどちらもSol Highなので相互排他とする。独立reviewはEngineerの作業終了後に開始する。同じ課題で同時に許すのは実装者と独立reviewerの二系統だけだが、model+tierが一致する場合は順番に実行する。Sol Max Reviewerも同tierで同時1体まで。Astraはreasoning effortにかかわらず全体で同時1体までとする。同じ課題を複数agentに競わせない。

security、public contract、migration、ownership/lifecycle、concurrencyやcorrectnessに重大な影響がある変更は、実装者の自己reviewだけで完了にしない。実装後にIndependent Reviewerが差分と検証根拠を独立して確認する。Sol Highを使う実装者や親agentの同tier作業が終わる前にHigh Reviewerを並行起動しない。

`.codex/agents/*.toml` はCodexがroleファイルとして自動検出し、別のrole登録表は不要。各ファイルのtop-level `name`、`description`、model設定とdeveloper instructionsを保持する。`.codex/config.toml` は子threadの並列上限を設定する。

起動前に利用するCodex実行環境のmodel catalog/APIでmodel IDとreasoning effortを確認する。未対応または確認できないIDは起動せず、実行環境と確認根拠を親agentへ報告する。似たIDを同一modelの別名と推測して置換しない。

設定仕様と環境ごとの検証範囲は[agent構成の記録](docs/internal/agent-routing.md)に残す。

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
