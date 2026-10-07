# Nagi 0.1.11 version/docs worker review

## 基点と対象

- Worktree: /tmp/nagi-release-0.1.11
- Base commit: 84373a2383a7cf5d5ef417dbfab084a95916fa08
- Base tree: 84d1696053a9cf5ef256b44fcd50afff91863c9d
- Version audit: /tmp/nagi-release-task-audit/17-version-reference-actions.json (105 update, 38 preserve, 2 review)
- 変更は公開README、DESIGN、AIガイド、公開Docs、対象サンプルREADME、および日英migration guideに限定。Cargo files、CHANGELOG、CI、compiler/runtime、Task source、docs/internal、benchmarksは編集していない。
- 作業中のstatusにあった docs/internal/handoffs/2026-10-06-task-release-0.1.11.md は親agentのファイルとして扱い、編集していない。

## 変更ファイル

- DESIGN.en.md
- DESIGN.md
- README.en.md
- README.md
- ai/README.md
- ai/language.md
- ai/skills/nagi-development/SKILL.md
- docs/README.md
- docs/async.md
- docs/builtins.md
- docs/editor.md
- docs/en/README.md
- docs/en/async.md
- docs/en/builtins.md
- docs/en/editor.md
- docs/en/error-handling.md
- docs/en/getting-started.md
- docs/en/language-guide.md
- docs/en/modules-and-rust.md
- docs/en/ownership.md
- docs/en/projects.md
- docs/en/roadmap.md
- docs/en/syntax.md
- docs/en/task-handles.md
- docs/en/types.md
- docs/error-handling.md
- docs/getting-started.md
- docs/language-guide.md
- docs/modules-and-rust.md
- docs/migration-0.1.11.md
- docs/ownership.md
- docs/projects.md
- docs/roadmap.md
- docs/syntax.md
- docs/task-handles.md
- docs/types.md
- test-nagi-code/application-examples/README.en.md
- test-nagi-code/application-examples/README.md
- test-nagi-code/application-examples/auth-boundary/README.en.md
- test-nagi-code/application-examples/auth-boundary/README.md
- test-nagi-code/library-examples/foundation-cli/README.en.md
- test-nagi-code/library-examples/foundation-cli/README.md
- test-nagi-code/library-examples/foundation-report/README.en.md
- test-nagi-code/library-examples/foundation-report/README.md
- test-nagi-code/library-examples/supervised-service/README.en.md
- test-nagi-code/library-examples/supervised-service/README.md
- test-nagi-code/library-examples/task-results/README.en.md
- test-nagi-code/library-examples/task-results/README.md
- test-nagi-code/low-examples/order-quote/README.en.md
- test-nagi-code/low-examples/order-quote/README.md
- docs/en/migration-0.1.11.md

## 内容と判断

- README、Docs index、AIガイド、DESIGN、対象例の版案内をNagi 0.1.11のリリース対象へ揃えた。0.1.11が公開済みだとは断定せず、公式GitHub Releases記録で入手可否を確認する文言にした。VS Code拡張0.1.13は別版のまま。
- Getting startedの表示例、版指定install command、archive名を0.1.11対象に更新し、公式Release確認後に使う説明を付けた。導入済みの最小版0.1.10と、従来例の0.1.9要件は維持。
- 明示move、Task結果handle、S1/S2を0.1.11対象として表記した。S1 PR #88はmain merge済み、S2は既存APIを使う実装が完了し、PR #90の4 OS CIは進行中と記した。Taskのlegacy spawn、inner Err、sticky fault、discard、Drop/closeの制約を維持し、新APIや安全性保証は追加していない。
- AI言語ガイドとnagi-development skillの標準module一覧にstd.taskを追加した。migration guidesはREADMEと日本語/英語Docs indexから参照し、日英間の内容を揃えた。英語guideから対象sampleへの相対linkはdocs/enから../../test-nagi-codeを使う。
- Foundation CLI/reportとorder-quoteの説明を更新し、native:行の成功pathと生成先.nagiの実行ファイルを案内する。NAGI_NATIVE_TARGET_DIRは共有dependency cacheの選択だけで、実行ファイルpathを決めないと明記した。固定target/path組立の誤説明を除いた。
- 既存SQL/byte API/HTTPの0.1.10履歴と必要版、その他sampleの0.1.9最小版、測定記録、VSIX 0.1.13は維持。Application sample indexもbyte inspectorの0.1.10要件とAxum APIの0.1.10検証を保持。
- Version auditのupdate参照は、変更対象公開ファイルに旧文面が完全一致で残っていないことを確認した。変更対象内のpreserve参照15件中13件は原文維持。types.mdの日英の古い「複雑なborrowはcheck後にRust buildで失敗し得る」という一般警告2件は、0.1.11対象のflow-plan coverageとNagi由来の生成型/lifetime拒否のcompiler defect扱いに合わせて文脈更新した。release/version対象外のpreserveとreview対象2件は変更していない。

## 検査と残る確認

- /tmp/nagi-site-venv/bin/python /tmp/nagi-s1-final-review/verify_markdown.py: 219 Markdown files、1980 local links、263 external linksはfetch対象外、errorsなし。相対link/anchorを検査した。
- git diff --check: pass。
- Version audit exact-text再確認: 変更対象公開ファイルのupdate参照残りなし。変更対象外のCHANGELOG.md、Cargo.toml、Cargo.lockには触れていない。
- コードtest・4 OS CIは実行していない。Docs-only変更のためで、S2 PR #90の4 OS CIはこの記録時点で進行中。
- 公式GitHub Releasesでの0.1.11公開recordはこの作業では確認していない。公開前の内容としてrelease record確認が必要。親agentのCI gate、独立readback、critical review、版更新とrelease判断に引き継ぐ。
