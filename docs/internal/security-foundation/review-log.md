# Security Foundation RFC: 指摘と検証

状態: 初回と安全性優先改訂の独立Sol Highレビューを別に記録。改訂SF-R05/06も修正後の独立再確認完了、未修正レビュー所見0。2026-10-08 JST。[RFC](rfc.md)・[計画](implementation-plan.md)・[現行調査](baseline-audit.md)。

## レビュー対象

基点main `62bbda9e8e8a9b07b0b2c1fd92057a9751c36fe3`。runtime/compilerの新実装はない。提案の内部整合、既存ADR/互換性/High-Low/trusted境界、実現性、テストと全体acceptance、保証の過大表示を独立Sol Highで確認する。初回はD1–D3未採用の互換性優先提案を対象にした。最新ユーザー指示後の改訂は下記別節で記録し、レビュー成功とruntime実装済みを混同しない。

## 初回指摘台帳（互換性優先案の履歴）

独立Sol Highの最終再確認でSF-R01–04すべて修正済み、未修正0。初回Critical/High指摘0、Medium2、Low1。修正後の再確認で既存の現状誤記Low1を追加。レビューはread-onlyで、runtime試験・脆弱性再現はしていない。

| ID/優先度 | 根拠と指摘 | 修正 | 状態 |
|---|---|---|---|
| SF-R01/Medium | 失効checkとnative admissionの順序が未定義。[session.rs](../../../runtime/src/sqlite/session.rs):535のasync enqueueと809以降のworker実行は別 | admissionを対象/内容にboundしたprivate一回execution permit発行の線形化点に定義。予約はadmissionではなく、失効/時刻検査とpermit発行は共有gate、gate内awaitなし。queue待ち/検査後の失効barrierと逆順oracleをSF01/05/06へ追加 | 独立再確認済み |
| SF-R02/Medium | Set-Cookieだけではcacheを禁止しない（取得RFC6265 §3、OWASP Session「Web Content Caching」）。旧append_headerは任意Cache-Controlを許す | 標準Session発行/rotation/認証状態・CSRF配布でfinalizerがno-storeを所有。競合public/max-age/304再利用を拒否しwireで確認。一般DTOの機密分類は保証しない | 独立再確認済み |
| SF-R03/Low | DESIGN.enの#99/CI状態が日本語と不一致 | 日英tableと現状段落をmain62bbda9/4 OS成功/0.1.11未収録へ同期 | 独立再確認済み |
| SF-R04/Low | DESIGN日英でCheckedProgramが未リリースとする既存誤記。公開003a594のchecked.rsとCHANGELOG 0.1.11節に収録根拠 | 両文をNagi 0.1.11収録済みへ同期 | 独立再確認済み |

レビューでは8機能、静的/runtime/trusted境界、High/保存Low/手書きLow/sealed plan、同task request proofと旧delegation、HEAD/OPTIONS/builtins、Origin/proxy、PR依存/budget/native/browser/4 OSにCritical/Highの矛盾は確認されなかった。実装の安全性証明やD1–D3採用承認ではない。

最終signature、renderer subset、依存と実socket対応、実browser/proxy/TLS、数値budget、performance、migration実行は今後のacceptanceで未確認。これらをレビュー指摘の未修正と混同しない。

## 検証

文書差分のlocal検証（2026-10-08 JST）:

- website: 98ページ、全local link/anchor/asset成功。新内部RFCは公開APIリファレンスへ未実装の使い方として載せず、DESIGN/roadmapから辿る。
- 初アプリ/SQLite tutorial: 日英code blockと既存native-tested sourceの一致を`--docs-only`で成功。今回nativeを新規実行した証拠ではない。
- CI policy: `python -m unittest discover -s scripts/ci -p 'test_*.py'`、59 tests成功。`git diff --check`成功。
- maintained Markdown（archived benchmarks/resultsを除く）246ファイル、2531 local link/image path、欠落0。path確認であり外部URLや全Markdown anchorの検証ではない。
- 最初の全tracked Markdown探索は312ファイルで、部分snapshot/独立reviewの保存コピー12ファイルの145リンクが元contextを参照し欠落。全12ファイルがmainから未変更と確認し、歴史artifactを書き換えず、maintained-doc成功数と分離した。新規欠落0。
- baseline source hash25件は固定mainの`git show`と一致。JSON台帳をparse確認。compiler/runtime/Cargo/公開API/版の差分0。
- 初回websiteはPython依存不足、二回目は`--out`が既存build-directory guardにより拒否。専用venvへ既存requirementsのpinを導入し、正規build配下で成功。失敗を新機能の契約RED/GREENに数えない。

現行main CI結果は[baseline.json](baseline.json)で、この差分の新CI成功と数えない。設計PR [#100](https://github.com/disnana/Nagi/pull/100)の最新head/ChecksはPR本文とGitHubを読む。文書だけでskipしたRust/4 OSは今回実行したとは報告しない。

## 安全性優先の改訂レビュー

最新指示に基づくD1単一HTTP/D2単一request-bound Grant/D3永続Sessionと[移行契約](decisions-and-migration.md)、日英RFC/plan/DESIGN/handoffを別の独立Sol Highがread-onlyで確認した。Critical/High指摘0、Medium1、Low1。runtime/攻撃PoC/第三者試験はしていない。初回SF-R01–04の完了を今回の独立reviewの代用にしない。

| ID/優先度 | 指摘 | 修正 | 状態 |
|---|---|---|---|
| SF-R05/Medium | 生存session数/行サイズだけではexpired/revoked情報を含む保存総量がboundedにならない | 生存数/保存総行数/行bytesを個別制限、cleanup未完insert拒否、失効情報有限保持/不在ID拒否。論理recordsとphysical page/WALを分離、journal_size_limitをhard cap扱いしない。少数反復/遅延cleanupのSF02 oracleを追加 | 独立再確認済み |
| SF-R06/Low | 同Txのsession世代predicateに同file配置前提がない。現行authorizerはATTACH禁止 | Session表/対象データを同SQLite file/Txへ置く前提、別file/store lookupで代替不可、ATTACH禁止維持を日英に追加 | 独立再確認済み |

改訂の文書検証: website98ページのlocal links/anchors/assets成功、onboarding/SQLite日英source一致（docs-only）、CI policy59成功、maintained Markdown248ファイル/2571 path欠落0（最終値はlocal-validation.json）、固定main source hash25一致、runtime/compiler/Cargo差分0。link script初回のregexはNagiのgeneric呼出しをリンクとして誤検出したため、Markdown parserで実リンクのみを検査した。契約テストの成功ではない。

先行head `56da8684a7c18d6331203e43f02dd4c0fde88b5b` checks37746386753/website37746386400はsuccess。Linux fullのlocked workspace tests、SQL/Task stage/location、High/保存Lowチュートリアル、HTTP integrations、examples、fuzz-smoke実step成功を読戻した。4 OS/package/JetBrains/releaseはskipで、新機能の実行証拠にしない。改訂head CIは別にreadbackする。

最終同期でbaseline-auditの旧Grant delegation維持案を調査開始時の履歴と明示し、最新の単一request-bound契約へ参照を接続した。固定source hashや現行mainの挙動を改変したものではない。
