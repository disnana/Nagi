# Security Foundation: 実装・PR・検証計画

[RFC](rfc.md) · [English plan](implementation-plan.en.md) · [現行調査](baseline-audit.md)

2026-10-10 UTC。**計画でありFoundation全体の完了記録ではない。** GitHub main `e609aba158921226a632f16d41eb8b0f4ad5aebd`にはSF00 #100とSF01 #101がmerge済みで、SF05のcanonical Query/Parameters実装とSF07 bounded reader変更も開発sourceに統合済み。SF02/SF03/SF04/SF06は未完了、SF07の横断budget acceptanceとSF08も未完了。正式0.2.0は未リリースで、公開0.1.11は変更されない。統合済みコードだけでは他のacceptance完了を意味しない。SF01[専用契約](sf01-contract.md)と結果、SF05[契約](sf05-contract.md)・[結果](../security-foundation-sf05-results.md)で個別の確認範囲と制限を記録する。最新ユーザー指示に従う[D1–D3判断・移行](decisions-and-migration.md)を基準とし、旧資料の互換性保留を再承認待ちにしない。各featureの公開signature/contractはRED追加前に固定する。意味論が一意な内部調査、依存比較、test harness設計は並行して進められる。

2026-10-10 14:04 UTCのGitHub readbackでは、main checks `38055073417` のLinux、4 OS package、4つのIDE Stable/EAP、common ZIPが成功し、websiteも成功した。`publish-release`はCHANGELOGに必要な `## JetBrains 0.1.3` 見出しがなく失敗し、0.1.3のtag/Releaseは作成されていない。公開処理の再実行は承認まで行わない。CodeQL run `38055084000`はC/C++ extractionで `Extraction failed: No source files found` と報告した。全check成功とは扱わない。

## 実装の分割と依存

以下はPR候補IDでGitHubの実番号ではない。各PRは最新main向けとし、未マージ依存がある場合のみstacked baseを明記する。先行PRの契約/必須CI/独立reviewが完了するまで依存する実装を完成扱いしない。merge、tag、正式release、破壊的branch変更を自動実行しない。

| PR | 内容と責務 | 依存/採用判断 | 完了oracle |
|---|---|---|---|
| SF00 | baseline、RFC、移行/完了条件、独立設計review | 最新main、安全性優先D1–D3の判断・移行 | source/CI/PR readback、Docs links/site、レビュー指摘追跡。新機能のGREENを主張しない |
| SF01 | security resource metadata、failure分類、AuthScope/単一Grant[P]、policy必須HTTP app/route、dispatcher lifecycle。予算/managed-header共通境界も固定 | SF00、D1/D2・公開API ADR | spoof/share/escape/wrong permission/同request照合、失効/一回execution permit発行の線形化と失効前後admission、正常/Err/panic/取消Drop、GET/HEAD/OPTIONS/404/405、dynamic route、旧入口のmigration診断/削除と移行後native |
| SF02 | credentials verifier adapterとCookie/Session store/rotation/失効。crypto/cookie library比較、秘密ログ制限 | SF01/SF05、D3・永続SQLite store/依存選定 | duplicate credentials、claims検証、session rotation/logout/expiry/容量、restart/crash/clock/multi-process世代競合、生存/保存総数上限と失効行cleanup遅延、同file predicate配置、物理容量とlogical boundの区別、store fail-closed/結果不明、発行/token応答no-storeと競合拒否、TLS cookie browser native E2E |
| SF03 | CSRFとCORSを別policyで接続、origin/proxy信頼、preflight、early response finalizer | SF01/SF02、credential source/外部origin契約 | public login/logoutを含むunsafe cookie request、token/origin、credentials/wildcard拒否、Vary/preflight、denial時handler未呼出とpermit解放 |
| SF04 | typed HTML subset renderer、URL attribute、CSP/nosniff、raw標準入口削除/response迂回拒否 | SF01、対応subset/managed-header契約 | text/attribute/URL context・unsupported要素拒否・再encoding・出力予算。browserで構造/挙動、High/Low native wire |
| SF05 | literal QueryとParametersを標準SQLiteへ統一、旧Db/dynamic入口削除、protected DB対象bind、管理SQL境界migration | SF01、新Query API契約。#99は既にmain | dynamic constructorのchecker拒否、literal/Parameters native、schema/shape/NULL/Outcome/authorizer旧回帰、protected predicate |
| SF06 | policy付き送信HTTP、URL/DNS/socket/proxy/TLS/pool対応の依存比較とadapter | SF01、client/URL/TLS依存選定 | checked addressと実socket対応、retry/pool/redirect不追従、size/deadline、取消解放。mockのみをGREENにしない |
| SF07 | 横断budgetとbounded rate limiter、cache/entry上限、観測とcleanup、必要なpeer/proxy境界 | 各機能のbudgetは各PR必須。SF01–SF06の実装を統合 | 偽時計/少数entry/小容量で上限/期限/解放、認証deny後と正常継続。既存frontend/HTTP/Actor/SQLite防御不変 |
| SF08 | migration guide、全機能アプリ、public API差分、日英Docs/examples/IDE、配布外native、最終独立review | SF01–SF07の4 OS/レビュー完了 | 下記全体acceptanceの全行に最新source headの証拠、未解決blocker0。リリースは別の明示承認後 |

推奨順はSF00→SF01→SF05→SF02→SF03、その後SF04/SF06を独立に仕上げ、SF07→SF08。SF04/06は別fileの調査なら並行可能だが、compiler stdlib/checked planを複数agentで同時編集しない。共通型/metadataはSF01で固める。HTTPのAxum全面置換は含めず、既存Hyper transport上で責務を分ける。

## 各PRの必須手順

1. 採用したsignature/ownership/lifecycle/失敗/上限と、旧APIとの差をcontract表へ固定する。
2. 正例・最小negative・runtime oracleを先に追加しREDを保存する。compiler負例はparse/name resolution成功後の正しいcheck拒否＋診断fragment/元行。API未定義、panic、parser/import失敗、別のlimitによる拒否を目的契約の成功にしない。
3. canonical registryとchecker facts/sealed plan、必要なruntime境界へ実装する。format名一覧やemitterのsecurity再推論を使わない。
4. High→Low→Rustと、独立再loadした保存Low・手書きLowをそれぞれcheck/build/runする。Low metadata、alias、shadow、wrapper、callback/phantom、native `@replace`統合の負例も確認する。
5. 対象回帰→必要なworkspace全回帰/fmt/clippy→bounded生成/mutation→native E2E→4 OS。新target/filterの0件実行は失敗とする。
6. API/制約/保証外/期待結果/診断と直し方をDocs日英・DESIGN・ADR・examplesへ同期し、code blockを実行可能なものと提案に分ける。
7. 独立Sol High review、指摘修正、影響する検査の再実行、最新headの必須CIをreadback。担当者の成功報告でレビューを代用しない。

新規crypto/URL/cookie/client crateはmanifest要求だけでなくlock解決版のsource、license/MSRV/features/OS/TLS/backend、security advisory/保守状態、技術的限界を記録する。crateの名前だけで安全と扱わない。依存比較と小さなconsumerで実接続/取消の実現性を先に確認する。

## 検証対応表

各featureに下記経路を登録するmanifestを新設する計画。現在のTask/SQLite runnerを参考にするが、expectedをparse-failへ変更したりnegativeをnative未実行成功にしない。security contract runner自体のstage/location/bad-oracleテストを先に持つ。

| ルート | checker正/負 | 診断位置 | build | 実行と観測 |
|---|---|---|---|---|
| High | 全契約の基準 | High/moduleのprimary行、対応のないRust spanは推測しない | 受理したsupport内caseは必須 | 本番と同じruntime・有限local fixture |
| High→生成Low→Rust | 最終Low/native統合checkも必須 | 同compileのHigh mapping | 通常CLI経路で必須 | Highの結果/Drop/解放と一致 |
| 保存Lowの独立入力 | High factsを持ち越さず再check | 保存Low自身の位置 | 正例の全feature | 独立CLI/nativeで同じ保証 |
| 手書きLow | High emitのtextをoracleにせず独立正/負 | 手書きLow自身の位置 | featureごとに必須 | 同じruntime contract、policy省略で迂回しない |
| native Low/`@replace` | 最終統合でsignature/factsを確認 | native/replace由来 | 受理/不一致を区別 | final runtimeに旧入口へdowngradeがない |
| Rust trusted adapter | public consumerでNagiとの型/ownership確認 | native Rustの位置 | locked Cargo・trait/Send/Sync | 実verifier/driver/store契約を別oracleで検証 |
| 配布外 | extracted packageでHigh/Low check | package内元位置 | bundled runtime/lock参照 | isolated cwdで全機能sample実行 |

## 防御的negative/adversarial試験

対象は所有するlocal fixtureのみ。小さな入力・人工clock・deterministic barrier・有限worker/queueで行い、第三者への攻撃、exploit/PoC、無限生成、資源枯渇、負荷/有料scanは行わない。新モデル/追加サービスへ秘密を送らない。

| 面 | 最小の検査と独立oracle |
|---|---|
| Auth | missing/fake/wrong marker/reuse/shared/field/Task/Actor/wrapper、同名別module、同requestではないGrant、未poll/pending/失効/再使用。拒否時protected operation呼出0、許可時対象実値を観測。queue予約待ち/検査後・permit発行前の失効barrierでnative enqueue0、逆順では既受理operationを観測しrollbackと呼ばない。SQL/client adapterでも実queue/socket境界へこのoracleを接続 |
| Route | policy欠落/型不一致、dynamic path/alias/mapper、GET→HEAD/明示HEAD、OPTIONS preflight/actual、builtinsが新appにないこと。anonymous/protectedをwire結果とhandler counterで区別 |
| Credentials | duplicate/混在/invalid credential、有限token/key、issuer/audience/exp/nbf/algorithmの不一致とtrusted key selection。秘密がDebug/公開errorにない。実verifier成功と型checkを別記録 |
| Cookie/Session | 同名cookieの曖昧性/属性/parse、CSPRNG failure、期限/時計/rotation/logout、atomic同時lookup/rotate、store停止/返信喪失、少数entryの満杯。Session発行/rotation/認証応答とCSRF配布のwire no-store、競合cache header/304再利用拒否。再起動/多instance制約も明示 |
| CSRF | unsafe cookie/public login/logout、token欠落/不一致/別session/rotation、Origin欠落/null/重複/不一致、header/form sourceの一意性。deny時body/permit解放とhandler未呼出 |
| CORS | same-origin/allow/deny、credentials＋wildcard、method/header、preflightのmetadataのみ、早期401/403/413/500、Vary。実browserによる読取可否と非browser transportを分ける |
| XSS | 対応text/quoted attr/URL slotの正しいDOM、unsupported element/attribute/context拒否、fragment再利用/二重encoding、NUL/Unicode/length、CSP/nosniff。危険script実行のPoCを作らず構造assertと小さなbrowser fixtureを使う |
| SQL | literal構造/Parameters separation、dynamic/format constructor checker拒否、誤bind/一文/column/NULL/authorizer、owner predicateとgrant対象。native DBの行/変更数をassert、旧Db/Tx制約保持 |
| SSRF | deterministic resolver/connector＋所有する小さなTLS endpointでURL/全address/mapped family/pool/retry/redirect/proxy policyを検証。拒否時socket接続0、許可時選択address/hostname/TLS一致。外部metadata等へ実際に接続しない |
| DoS/lifecycle | 1～数個のentry/permit/worker、境界±1・fake clock・cancel/Err/panic/shutdown。admission/lease/queue/native worker live数とactual join、次正常requestを観測。単なる通知/timeout返却を全終了と数えない |

browser試験は実TLSと制御されたoriginだけでCookie/SameSite/CORS/CSRF/DOMを確認し、Chromium/Firefox/WebKitの実行target/版を記録する。browser binaryを用意できない環境は未確認で、unit/mocksで置換して完成にしない。実装PRの4 OSは現行Linux/Windows/macOS ARM/macOS Intelへ追加し、browserはLinuxの専用job＋明記したsupport matrixにする。

## 維持する回帰とコスト

[移行表](decisions-and-migration.md#移行対応表と検証)に記録したHTTP/auth/SQL/raw-outputの旧受理だけは、移行診断と移行後同等業務動作の対へ変更する。仕様変更に無関係な既存のownership/move/view、resource registry、sealed facts、auth boundaries、Task全契約と16native oracle、旧spawn/Supervisor/HTTP、SQL/SQLite public/Tx、frontend limits、CLI生成/cache/output、editor trust、examples、配布gateを維持する。少なくとも次を各コードPRの変更に応じて実行する。

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -p nagic --example fuzz-smoke
python scripts/verify_compiler_contracts.py
python scripts/verify_task_handle_contract_inputs.py
python scripts/verify_application_examples.py
python scripts/verify_onboarding_examples.py
python scripts/verify_sqlite_example.py
python website/build.py
```

socket/Cargo依存を用意し、infra失敗と契約失敗を別記録。文書だけのSF00でRust全suiteを新規実行する必要はない。4 OSの過去main成功と新head成功を混ぜず、skipしたjobを実行成功に数えない。

費用は同じ保証/入力/依存/targetで生成Nagiと手書きRustを比較する。auth/verifier/session/renderer/clientのallocation/保持entry、取消後保持、Future size、compiler check/Low/check、compile/binary、有限通常caseの時間を測る。実施条件/worker heap除外/cache/分散/限界を併記し、securityを無効化して数字を揃えない。秘密やユーザーデータをartifactへ保存しない。

## 全体完了条件とrelease gate

0.2.0 Security Foundationとして完成を宣言するには、以下の全行にfeature PR・最新head・test/report/artifactを結び付ける。未実装を「制約」として隠してrelease gateから除外しない。後続へ明示的に外した任意機能は、RFCの対応範囲を修正し採用判断を記録する。

- D1–D3の判断と全旧入口export/migration inventory、公開API/失敗分類/所有権/失効/route/credential source/上限をADRとpublic API差分に固定。現行/実装方針/未対応の表が日英で一致。deprecated並走・policyなし標準入口・raw HTML/header/SQL bypassが残っていない。
- AuthScope/認可、route網羅、CSRF、typed HTML、SQL構造/bind、実接続SSRF、CORS、Cookie/Session、bounded DoSの各positive/negative/normal/Err/panic/cancel oracleがGREEN。受理後の新check/build mismatchなし。
- High/生成Low/保存Low/手書きLow/native統合/配布外の対応表を埋め、元位置、wire/DB結果、handler未呼出、permit/lease/native終了を実観測。0件filter、parse拒否、compile-onlyをnative成功に数えない。
- 既存move/Task/spawn/HTTP/SQLite/Actor/CLI/editor/examplesの全回帰とfuzz、最新source headの4 OS必須CI/website/配布gateが成功。全skip/ignored/infra制約を列挙。
- 配置前提を含むbrowser/TLS/proxy/session/SSRF native統合試験が成功。mockや別保証のbare HTTP比較で代用しない。
- 日英Docs/DESIGN/ADR、実行できるapp例、migration guide、diagnostics/直し方、performance/cost、dependency/license、changelog/release notes案が一致。
- 独立最終レビューを実装者と分離し、contract/lifecycle/concurrency/error/security/compatibility/coverageを確認。重大/高優先の未解決指摘0。未確認は未確認として残し、security claimに含めない。
- release blockerと承認状態を記録。**ユーザーの明示承認前にversion bump、tag、正式releaseを行わない**。承認後は既存のimmutable release/4 OS package/外部抽出検証/installer手順を使う。

## 独立レビューと引継ぎ

通常実装/fixture/DocsはまずLuna Maxを検討し、security/compiler/runtime境界はSol High。独立reviewもSol Highを実装者終了後に起動する。設計上Highで解けない点だけSol xHighへ昇格する。Max/Astra常用、同問題の競争、重複reviewをしない。親を含む通常上限3、同model/tier同時1。

指摘台帳にはID、優先度、source/contract根拠、変更/不採用理由、検証、reviewer再確認、残blockerを記録する。[review-log](review-log.md)をこの段階の正本とし、実装PRでは専用artifactへ分ける。進捗とhandoffはsource head/採用判断/完了PR/未完了/次順序/保証外を残し、過去snapshotを書き換えない。
