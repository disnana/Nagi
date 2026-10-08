# Independent newcomer Docs QA

初回レビュー対象: `/workspace/Nagi-docs-onboarding` のhead `2bae879a950e51ba9d3e38f4c515b0234f04b894`。初参加者がfirst-appと本体contribution手順を記載前提だけで進められるか、索引とwebsite導線、日英、例・コマンド・CIの対応を読み取り専用で確認した。

## 初回指摘

- **P2 — first-appの誤り修正を読者が再現できない。** `docs/first-app.md:81-89` と英語版は比較が`<`だった場合を仮定するが、掲載コードは`<=`のため、書かれた順に試すだけでは失敗が起きず修正手順に入らない。比較を一時的に`<`に変えて`1000`で誤判定を見てから`<=`へ戻す手順を明記する。
- **P2 — Docs-only PRでtutorial検証が省略される。** `scripts/ci/changes.py:43-44,103-109` はDocs-onlyを`full_checks=false`にし、`.github/workflows/ci.yml:41-43,94-95` のonboarding verifierはそのLinux job内にある。Pages PR jobはリンク検査だけでNagi codeをcompileしない。Docs-only PRでもJP/EN掲載codeをnative-tested sourceと照合する軽量検査が必要。
- **P3 — 日本語版が実行時エラーを「診断」と呼ぶ。** `docs/first-app.md:112` の`abc`/`-1`は実行時エラーだが「診断」と記載。英語版は“error”。compiler診断と区別できる「実行時エラー」に揃える。
- **P3 — 公開Docsから内部監査記録へリンクする。** `docs/README.md:77` と英語版が`docs/internal/docs-onboarding-audit.md`へ読者を送る。サイトでは公開ページ化されずGitHub sourceへのリンクになる。公開索引からリンクを外し、記録はinternalのままにする。
- **P3 — 貢献ガイドの導入と具体例が不一致。** `docs/contributing.md:5` は小さなcompiler修正を追うと説明するが、具体例は`docs/contributing.md:96`で実装を変えずnullable fixtureを追加する。導入をfixture追加の手順に合わせるか、実装修正例を追加する。

## 初回確認

JP/ENの公開Docs route集合とwebsite navにfirst-app/contributingがある。Pool/Txを利用可能とは案内していない。first-appは別scratchでcompiler 0.1.11の`check`と通常入力`700`を再実行し、`within budget`を確認した。fixture suite、full website build、Windows/macOS、IDE、installer、fork push/PRはこの初回独立reviewでは再実行していない。repository sourceを編集していない。
