# 引継ぎ: 0.2.0 Security Foundationの調査/RFC

2026-10-08 JST。[RFC日英](../security-foundation/rfc.md)、[実装計画](../security-foundation/implementation-plan.md)、[調査](../security-foundation/baseline-audit.md)、[review台帳](../security-foundation/review-log.md)を起点にする。まずroot AGENTSを読む。

## 現在の状態

- 基点main `62bbda9e8e8a9b07b0b2c1fd92057a9751c36fe3`、tree `a1bc0e601ea0f06a1d92940bc1c87da28a885ace`。#99は外部でマージ済み。確認時open PR0。0.1.11 tagは`003a594de086383100016b7c75466da37705646c`で、SQLite公開APIは未収録。
- branch `docs/security-foundation-020`、worktree `/workspace/Nagi-security-foundation`。調査/RFC/計画・日英DESIGN/roadmapの現状同期だけ。compiler/runtime/Cargo/公開API/版は変更していない。
- AuthScope/CSRF/XSS/SQLi/SSRF/CORS/Cookie/Session/DoSの静的/runtime/trusted契約、High/Low/sealed境界、移行とSF00–SF08の依存/検証/全体acceptanceを作成した。0.2.0実装済みという意味ではない。
- 独立Luna MaxのDocs/tests/build/editor棚卸しと親のruntime/compiler調査を実施。独立Sol Highの設計reviewでCritical/High0、Medium2（失効/admission順序、session/token cache禁止）、Low2（日英CI現状不一致、CheckedProgram公開版の旧表記）。4件とも修正し、独立Solで再確認済み。未修正レビュー所見0。具体的な根拠はreview台帳を正とする。

## 次の順序と判断

1. 新セッションではmain/open PR/RFC branch実headとCIを読み戻す。この引継ぎのcommitを本文へ自己参照させない。
2. **D1–D3は未採用の公開判断**。推奨D1はpolicy必須new App＋legacy deprecated（全旧HTTPを自動保護しない）、D2は旧Principal/Grant delegation維持＋same-task AuthScope/ScopedGrant、D3はserver Session store＋host-only cookie（初版bounded memory/atomic adapter、永続/多instance要件を確認）。ユーザーが採用したらADRへ固定し、同じ大枠を再質問しない。
3. SF01のcanonical metadata、失敗分類、execution permit/lease gate、policy付route/handler/managed headersを契約REDから実装。queue予約/Future生成はadmissionでなく、一回private permit発行が失効との線形化点。受理済み効果はrollbackしない。
4. SF02 Cookie/Session/verifier→SF03 CSRF/CORS→独立SF04 renderer/SF05 Query/SF06 policy付きclient→SF07横断budget→SF08全機能native/移行/独立最終review。各featureに有限budgetと日英Docs/native/4 OSを含める。
5. 新clientのURL検査だけをSSRF完成にしない。実接続/DNS/retry/pool/TLS/proxyまでoracleを用意する。Session/token発行のno-storeは新app finalizerが所有する。raw HTML/dynamic SQL/旧HTTP/Rustは明示した保証外境界。

## 検証と禁止事項

現行mainのchecks37722308221・website37722307935成功、Linux full＋4 OS package/nativeのstep結果をreadbackした。これは新RFC差分のCIや新機能の検証ではない。新Docsのリンク/site/日英sample整合/CI policyの結果はreview台帳へ記録する。未実装機能のruntime/browser/TLS/依存選定/performance/migrationは未確認。

既存move/Task/spawn/Tx/SQL取得budget/High/Lowを再実装・変更しない。新構文/region/effect/任意Rustのsecurity解析、SQLite全面改修、HTTP全面置換を追加しない。安全な有限local防御試験のみで、攻撃PoC/第三者接続/資源枯渇/有料scanは行わない。merge・version bump・tag・正式releaseは実行していない。正式0.2.0とtag/破壊的branch変更には別途明示承認が必要。
