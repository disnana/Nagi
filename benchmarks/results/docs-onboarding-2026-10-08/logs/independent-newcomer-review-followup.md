# Independent newcomer Docs QA follow-up

再確認対象: `/workspace/Nagi-docs-onboarding` のbase `2bae879a950e51ba9d3e38f4c515b0234f04b894` に対するworking差分。確認は前回の5指摘に限定。sourceは編集せず、記録だけこのscratchに作成した。

## 指摘の解消状況

- **P2 — first-app再現手順: 解消。** `docs/first-app.md` と `docs/en/first-app.md` は両方とも、`<=`を一時的に`<`へ変える明示指示、`check`と`1000`入力のコマンド、誤った`over budget`結果、その後の`<=`復元と3境界値確認を記載。scratch記録`boundary-regression-review.log`にも失敗と修正後の成功がある。
- **P2 — Docs-only CI検証: 解消。** `.github/workflows/pages.yml` がPR時に `scripts/verify_onboarding_examples.py --docs-only` を実行する。`scripts/verify_onboarding_examples.py` はHigh code blockがJP/EN各1件で、`examples/tutorial/first_app.nagi`と完全一致することを確認してからdocs-onlyで終了する。例source・verifier・CI routingが変更されるPRは`changes.py`上full checksへ進む。軽量検査はこの場でも実行し、`Onboarding: Japanese/English tutorial code matches the native-tested source`を確認した。
- **P3 — runtime errorの日本語用語: 解消。** JP/ENとも実行時エラーとcompiler診断を区別し、前者の回復先と、アプリ起動前のbuild診断の確認先を案内する。
- **P3 — public indexからinternal auditへのリンク: 解消。** 日英READMEから監査記録へのリンクが削除され、contribution guideへの導線だけが残る。
- **P3 — contributing introとfixture例の一致: 解消。** 日英introがnullable conformance fixture追加の具体手順を説明する表現になった。

## 確認根拠と範囲

- `python scripts/verify_onboarding_examples.py --docs-only` 成功。
- `git diff --check` 成功。
- `boundary-regression-review.log` は一時的な`<`で入力1000が`over budget`になり、`<=`復元後に700/1000/1001が期待どおりになった記録。
- `ci-routing-tests.log` は59件成功の記録。これは既存scratch logを読み、今回再実行したものではない。
- headは`2bae879a950e51ba9d3e38f4c515b0234f04b894`のままで、確認した差分はworking tree上。

Windows/macOS、IDE、installer、fork push/PRは未確認。fixture conformance suiteとwebsite buildはこのfollow-upでは再実行していない。
