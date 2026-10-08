# SF01引継ぎ（2026-10-08）

root AGENTS.mdを最初に読む。[結果正本](../security-foundation-sf01-results.md)、[review台帳](../security-foundation/sf01-review-log.md)、[公開契約](../security-foundation/sf01-contract.md)、ADR013、日英security/migrationを優先する。

## 現在

- #100はユーザー承認でmain10655eaへマージ済み。a97とtree075ef292が一致し、SF00検証sourceを維持。
- #101はmain向けDraft、branch feat/security-foundation-sf01。production/check修正source d30361c、VS Code移行を含むsource f9ec01d、まだmerge/版/tag/releaseなし。実際のPR head/main/tree/Checksを毎回読戻す。
- public AuthScope/Grant/Policy/compiler/check/High→Low/seal/Rust/runtime接続済み。先行旧受理RED/期限後Err REDと独立review copy/share REDを保存。
- 高/保存低/手書き低native、typed callback/migration/@replace元位置、gate9/security HTTP11/public runtime1、Task8、既存回帰は成功。全workspace100 blocks/997 passed/failed0/ignored1、fmt/clippy/fuzz/site/examples成功。最終Sol High独立再確認は完了・blocking所見なし。source f9ec01dの4 OS/全必須CI（checks37771932580、website37771932126）は成功。結果・artifactの後続文書headはPR Checksを別に読戻し、同じproduction tree/hashであることを照合する。
- costはd127時点の62 requestでNagi/Rust同じdispatcher比較、Future/layout/allocation可視区間だけ。private lease/full Future/production verifier/DBは未測定、cache/同時build lock待ちあり。

## 重要判断

D1–D3を互換併存へ戻さない。policyなしstandard HTTP、無期限proof、unchecked parts/生subject issuer、raw HTML/managed headers迂回なし。Grant invalidationとprivate one-shot permitは同gate、native予約≠admission。permit後の取消はrollbackではない。任意trusted Rust verifier/authorizer/submit同期enqueue・bound targetの正しさを静的証明とは呼ばない。

新nonCloneとnonsharedを用途別に区別。CopyとCloneを混同しない。fn signatures/native protocol/phantomを実payloadにしない。Task/Txは既存SameTask/ownership/actual joinを維持する。旧Task HTTP nativeのsignature5箇所はexplicit public Policyへ移行し元oracle維持。

## 次の順

1. #101の結果・artifact追記headの最新PR Checksとproduction hashを読戻す。実装source f9ec01dの4 OS/全必須CI・独立Sol Highは完了。Draft維持・merge指示待ち。
2. 新規作業の指示があるときSF05 literal Query/Parameters/protected same-file Tx predicate、SF02永続Session、SF03 CSRF/CORS、独立SF04 typed HTML/SF06 outbound、SF07/08。現在SF02–08を完了扱いしない。
3. 型/移行元行はHigh/保存Low/手書きLow/native replace、native effect/wire/終了を対応表へ埋める。parse/import/違うlimit/未実行CIをGREENにしない。

0.2.0正式公開/tag/version bumpはユーザー明示承認まで禁止。PR #100の承認を他PR/releaseの承認として扱わない。古いPR100未マージ/実装未着手の引継ぎはその時点の履歴であり現状の正本ではない。
