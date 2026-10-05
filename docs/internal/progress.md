# コンパイラ・Rust境界の進捗

## PR0: 設計監査と段階計画

2026-10-05。基点main `8f6cc6cf7d7c08811736325263618cbea19314b8`。PR #76はユーザー側でマージ済みで、head `13b59aa`とmainのtreeが一致することを読み戻した。

### 実施内容

- 依頼全文、AGENTS、DESIGN日英、invariants、pipeline、ADR 001〜005、#76結果と関連実装・テストを照合した。
- Sol 2人が、Phase 1のchecked/provenance境界と、Phase 2〜4のbuild/resource/DBを分担して読み取り監査した。監査だけで成功を主張せず、根拠ファイルと未検証範囲を記録した。
- [全体計画](compiler-rust-boundary-plan.md)にGuarantee Register、owner、checked入力、provenance、error分類、各Phaseのacceptance・互換性・性能・Stopをまとめた。
- [Q-001](open-questions.md#q-001-同じアプリの識別と実行ファイルの世代を分ける)に、既存binary path期待とgeneration isolationの衝突、選択肢、推奨移行案を記録した。
- compiler/runtime、依存、生成ファイル、既存test期待、CI設定は変更していない。版更新・merge・releaseも行っていない。

### Acceptanceと検証

設計・実装状況・予定保証を区別する。PR0は文書のみなので、新しいcompiler conformanceの成功は主張しない。

- `python -m unittest discover -s scripts/ci -p 'test_*.py'`: 51成功。
- website build: 既存website用venvで90ページを生成し、local links/anchors/assetsを検証した。通常Pythonにはmarkdown-itがなく失敗したため、既存venvを使用した。出力はbuilderが許可する`build/boundary-plan-site`へ置いた。
- 変更7文書の相対リンク・anchorは171件を確認し、欠落なし。`git diff --check`も成功。
- [PR #77](https://github.com/disnana/Nagi/pull/77)の初回head `06c20ca`は[checks run 37300208409](https://github.com/disnana/Nagi/actions/runs/37300208409)・[website run 37300207891](https://github.com/disnana/Nagi/actions/runs/37300207891)が成功。PRのDocs-only比較でRust/native/editor/releaseはskip、change detection・release plan・merge gate・siteが成功。skipを新たなRust検証として数えない。
- Q-001承認を反映したhead `b156e05`も、[checks run 37300934374](https://github.com/disnana/Nagi/actions/runs/37300934374)・[website run 37300933963](https://github.com/disnana/Nagi/actions/runs/37300933963)が成功。更新pushのchecks/siteも成功を読み戻した。
- 新branchの初回pushは比較基点がなく、既存fail-safeによりLinux全suiteも起動した。PRの文書差分判定とは別で、これを新しいcompiler変更の検証と取り違えない。
- compiler/runtime/依存の変更がないため、今回の文書確認をRust build・4 OS・runtimeの新しい保証に数えない。

### 新しい反例とGuarantee Registerへの影響

新たなNagiプログラムのcheck/build反例を実行して発見したフェーズではない。見つかったのは、`shared_target.rs`がapp identityとgeneration pathを同一視する既存期待と、新依頼の契約の衝突である。

Guarantee Registerの「現在」は既存のownerを維持する。G-SEALED/G-GENERATION/G-TXは予定で、まだ保証していない。Auth ScopeはPhase 5以降の方向のみ。Rustへの最終borrow/trait/Send/Sync委譲は変更しない。

### 未解決事項・次Phase

Q-001はユーザーがAを承認した。app identity維持、generation別のside-by-side生成、build成功後のatomic latest更新を採用する。旧generationはbuild時に上書き・削除・killしない。承認済み設計に伴う内部path等のtestは理由を記録して更新できる。公開意味論・利用者契約・High/Low・登録保証・security/lifecycleの期待変更は引き続きStop。

計画とworking rulesへ反映済み。PR0の更新CI成功を確認し、Phase 1のfailing testsから再開した。Phase 2〜4を同時に実装しない。

その後ユーザーが#77をmainへマージした。main `ded4c44cd3ebf984b382995322cefc769b4a6cb3`のtreeは承認済みhead `b156e05`と一致することを読み戻した。Phase 1のPRはこのmainをbaseにする。

## Phase 1: 最終check済み入力の封印

作業中。PR0とは別branch `refactor/checked-program-boundary`で進める。mainへのmerge・版更新・releaseは行わない。

### 実装前の観測

`emit::rust(&Program)`の禁止を表すcompile-fail testを先に追加した。旧APIではコンパイルが成功し、`cargo test --locked -p nagic --doc`が「compile-failがコンパイルできてしまった」と失敗した。commit `3f76c2b`に保存した。この失敗をsealed APIで解消する。

既存callerは最終factoryへ移行する。内部factsを故意に破損するテストだけは再checkさせず、欠落・改変を検知するoracleを保つ。通常fixtureのUser Low扱いは既存生成比較のbridgeであり、実ファイルprovenanceの検証とは分ける。

[ADR 006](adr/006-sealed-codegen-input.md)に採用・不採用・保持する意味論を記録した。

### ローカル検証

最終factoryと封印APIを実装し、生成側のcapability/view/storage等の判断を封印時へ移した。既存fixtureはfactoryを通すhelperへ移行し、assertは維持した。内部破損のnegative oracleだけは再checkを行わない。

全suiteは89 suite・784成功、clippyも成功。SQL engineなし8成功、256生成caseを2つのseedで検査、10,000 mutation/128 native caseも成功。VS Code 196、HTML viewport 5成功。旧版の17正例から得たLow/直接Rust/保存Low Rustの51ファイルはbyte一致した。

最初のHTTP実行は通信制限によるloopback bind失敗、最初のeditor実行はcompilerのPATH未設定で失敗した。同じテストを必要な環境で再実行し、期待を弱めず成功した。詳細・発見した封印の穴・性能条件・保証の限界は[結果](checked-program-results.md)に記録する。

[PR #78](https://github.com/disnana/Nagi/pull/78)をmain向けに作成した。最初のCIではcompiler変更によるLinux/JetBrainsが起動したが、4 OS配布検証がskipされた。release planが版更新と配布設定だけを条件にしていたためで、成功とは数えない。compiler/runtime/Cargo入力にもNagi配布検証を適用する回帰と条件を追加する。版が変わらないときに公開しない規則は維持する。

CIの4 OS・JetBrainsは未確認。Phase 2以降の実装、mainへのmerge、版更新、releaseには進んでいない。
