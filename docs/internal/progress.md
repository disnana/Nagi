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
- 変更7文書の相対リンク・anchorは171件を確認し、欠落なし。`git diff --check`も成功。PR CIは公開後に記録する。
- compiler/runtime/依存の変更がないため、今回の文書確認をRust build・4 OS・runtimeの新しい保証に数えない。

### 新しい反例とGuarantee Registerへの影響

新たなNagiプログラムのcheck/build反例を実行して発見したフェーズではない。見つかったのは、`shared_target.rs`がapp identityとgeneration pathを同一視する既存期待と、新依頼の契約の衝突である。

Guarantee Registerの「現在」は既存のownerを維持する。G-SEALED/G-GENERATION/G-TXは予定で、まだ保証していない。Auth ScopeはPhase 5以降の方向のみ。Rustへの最終borrow/trait/Send/Sync委譲は変更しない。

### 未解決事項・次Phase

Q-001の回答待ちで停止する。依頼の「既存テストの意味論上の期待値を変更しないと通らない」というStop条件による。回答・計画反映・PR0 CI成功後、Phase 1のfailing testsから再開する。Phase 2〜4を同時に実装しない。
