# SF01: request-bound権限とpolicy必須HTTPの結果

2026-10-08。[PR #101](https://github.com/disnana/Nagi/pull/101)はmain向けDraft。SF00 [#100](https://github.com/disnana/Nagi/pull/100)はユーザー承認でmain `10655ea7299d775235ab6585e5ab8e321f121593`へマージ済み。SF01のmerge・版/tag/releaseは行っていない。**この記録はSF01の実装・検証を対象にし、Security Foundation全体の完成を主張しない。**

公開契約: [ADR013](adr/013-request-bound-auth-and-http-policy.md)、[API/境界](security-foundation/sf01-contract.md)、[認証日英](../security.md)、[移行日英](../migration-0.2.0.md)。原ログ/source hashは[検証artifact](../../benchmarks/results/security-sf01-validation-2026-10-08/README.md)。コストは[別条件記録](../../benchmarks/results/security-sf01-2026-10-08/README.md)。

## 実装の責務

- canonical registry/check: AuthScope/Grantのopaque・SameTask、Policy[S,A]とhandler/verifier/authorizerのsignature、非Clone/非shared資源の用途別payload検査。P phantom/関数署名/Policy A protocolと実所有payloadを区別する。
- High→保存Low→最終check/seal→Rust: High factsを保存Lowへ盲信して持ち越さず、`@replace`とaliasも最後に再確認。旧RoutePlan/needs_serverと暗黙global serve生成は削除。
- runtime: private finite lease owner、同gateで失効/現在時刻検査と一回execution permit発行、native予約とadmissionの区別、正常/Err/panic/timeout/cancel/Drop。全standard routeは明示Policy必須。
- Failure: security Failure6種は安定status/message/challenge、業務Errはmapper、TaskFailure/旧spawn故障をflattenしない。共通security budgetはverifier＋authorizer全体で一回。
- response: head-only security snapshot、body前policy、body後再expiry検査、HEAD fallback、404/405/早期エラーの基本finalizer、reserved framing/security headers、旧raw HTML入口の移行拒否。

## 契約と実行対応

| 契約 | High | 独立保存Low | 手書きLow/native統合 | 実Rust/native観測 |
|---|---|---|---|---|
| 旧decorator/global serve/Principal/policyなしroute | check migration＋元行 | parse/resolve成功後check＋Low元行 | 手書きLow同診断、alias/@replaceも拒否 | 旧public runtime/glueは削除、移行後HTTP例を実行 |
| Policy state/output、named async verifier/authorizer、mapper | check | 再check | `@replace` signature不一致拒否 | 正例はnative route/dispatcher |
| nonClone/nonshared wrapped resource | RED→check診断＋元行 | 再check＋Low元行 | 手書きLow check/build・PATH空、@replace拒否 | fn pointerの戻り値をpayloadと誤拒否しない3経路native |
| nominal Grantの生成/受取/対象 | checkerとextern宣言 | 独立load/check | 手書きLow | 同一std dispatcherでsubject42/target9→51、missing/invalid401、nosniff |
| move/同task delegation/Task escape | auth_boundaries/check | 同契約 | auth hand-Lowと既存Task corpus | Task/旧spawn/Tx回帰を別に維持 |
| permit前後失効/Drop | Nagiでnative private状態を偽造しない | 同左 | Rust native9gate群 | capacity待ちの失効、逆順受理、poison/expiry/counter |
| HTTP security lifecycle | 実アプリ・native adapter | saved app | Rust dispatcher11群＋公開runtime1群 | Failure6/wire/HEAD/Pending/構築・pollpanic/mapper/未poll/body中expiry |
| 旧HTTP/認可/CRUD業務 | 移行後app | 独立saved app | standalone supervisor Low | アプリ19・library15・HTTP107・CRUD21の実行 |

SF01 nativeは実socket/実Tokio/private leaseでありstub成功ではない。無条件「全プログラムが安全」を保証する表ではない。handler/auth callbackの任意Rust内部はtrusted。

## REDと修正履歴

初回compiler4契約は旧受理を確認してRED。最初のfixtureは既存builtin名/Low構文の誤りがあり、訂正版parse/resolve成功のREDだけを目的証拠とした。runtime API未定義のcompile REDは意味論oracleの成功ではない。手書きLow正例の最初のclass/case構文誤りも失敗ログを保持し、訂正後native成功と分離する。

non-yielding verifierが期限後Errを返すと403になるREDを保存し、全Ready結果の共通deadline guardで504へ修正。同期処理の強制停止や副作用rollbackは保証しない。

独立Sol HighレビューのP1/P2は[台帳](security-foundation/sf01-review-log.md)へ記録。nested copyは既知foreign非CloneをNagi checkで拒否、wrapper/field/shared App stateは既知nonshared検査へ修正。Task HTTP adapter5箇所は明示public Policy＋unit第3引数へ移行し、旧業務Err/terminal故障/shutdown/parent Dropのassertionsは維持した。

## 検証の状態

- targeted: compiler SF01 10群、native3経路1群、Task8群、manual Clone2群成功。runtime gate9＋HTTP security11＋公開runtime1群成功。
- fmt/clippy、fuzz固定seed1000 mutation/16 bounded native case（panic0）、website102ページ、maintained MD261/2649 path欠落0成功。外部URLをリンク検査で取得していない。
- 移行後例: application19実行、library15検査、HTTP integrations28/72/7、web-demo21成功。保存JSON/logとCI再実行を照合する。workerの口頭報告だけを原ログとして扱わない。
- 全workspaceは旧signature取り残し等で中断した各logを保存。修正source d303の`cargo test --locked`はexit 0。100 result block・997成功/failed0/ignored1（native Task cost専用）のログ集計。子プロセス/compile-fail Docも含むため独立contract件数と混同しない。cache使用実行でclean buildとは呼ばない。
- d303のCIは旧HTTPスニペットと標準resource期待のVS Codeテスト5件で失敗。HTTP callbackをPolicy付きに移行し、標準型一覧にPolicyを追加したsource `f9ec01d`ではVS Code196件成功。sandboxの同期spawn EPERMはnetwork権限付き再実行と分離して保存した。source `f9ec01d`の[checks 37771932580](https://github.com/disnana/Nagi/actions/runs/37771932580)・[website 37771932126](https://github.com/disnana/Nagi/actions/runs/37771932126)は成功。Linux全package・Windows x86_64・macOS ARM64/x86_64で全対象step、配布外検証、merge gateを確認。各platformでcompiler10/native三経路1/public runtime1/auth9/HTTP11が実行され、9/11件数guardも成功。publishはskipで、公開成功へ数えない。
- 最終Sol High独立レビューはf9ec01dで完了。元負例・capability unit・compiler/native/Task/manual Clone/runtime auth/HTTPを再実行し前指摘を閉鎖。全suiteの独立再実行ではない。詳細はreview台帳と原ログを参照。

## コストと限界

source head d127の同じ標準authorized dispatcherを使うgenerated Nagi/手書きRust policyを有限62 loopback requestで測定した。Future88/80B、AuthScope16B/Grant24B、同期Grant mint allocation0を観測。clone/shareのchecker修正後の比較計測ではない。runtime/generation条件は変えていないが、測定headと最終compiler headを同一と呼ばない。

shared cache、Nagi→Rust順、他workspace buildとのlock待ちを含むのでcompile wall timeを制御された速度比較とは呼ばない。private outer dispatcher/lease全heap/refcount/全寿命、thread外allocation、production verifier/DB/clientは未測定。小さい通常入力のlatencyからthroughput/SLOやsecurityの優劣を推定しない。

## 残る工程

次はSF05のliteral Query/Parametersと保護対象predicate、続いてSF02永続Session/crypto比較、SF03 CSRF/CORS、独立SF04 typed HTML/SF06 outbound HTTP、SF07横断budget、SF08統合。旧DbはSF05まで現行契約、raw HTML UIの同等typed migrationはSF04で未完。Cookie/session/CSRF/CORS/production crypto/browsers/TLS/送信HTTPを今回完成と呼ばない。

任意Rust verifier/authorizerとsubmit callbackの正しさ・同期enqueue・bound target、受理後副作用rollback、non-yielding停止、panic=abort/OOM/double panic回復、global session revokeはSF01の静的保証ではない。gate contentionの失効側はprivate active遷移のoracle、unpolled testはownerを明示dropするoracleで、任意scheduler順の普遍証明とは呼ばない。
