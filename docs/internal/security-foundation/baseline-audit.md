# Security Foundation: 現行境界の調査

調査日: 2026-10-08 JST。これは設計用の静的棚卸しで、脆弱性一覧・全経路の安全性証明ではない。[RFC](rfc.md)・[実装計画](implementation-plan.md)の根拠とする。

## 対象と手順

- main: `62bbda9e8e8a9b07b0b2c1fd92057a9751c36fe3`、tree `a1bc0e601ea0f06a1d92940bc1c87da28a885ace`。#99のSQLite公開対応までマージ済み。
- 公開Nagi 0.1.11: tag `nagi-v0.1.11`、commit `003a594de086383100016b7c75466da37705646c`。mainのSQLite公開APIはこの配布物に含まれない。
- GitHubのopen PR一覧は確認時0件。過去引継ぎのdraft/pendingを現状と読み替えない。
- root AGENTS、SECURITY日英、DESIGN日英、言語契約、ADR 001/004/006/008/010/011/012、compiler pipeline、関連sourceと正負テスト、CI/配布/エディターを読む。Luna Maxの独立したDocs/tests棚卸しと親のruntime/compiler調査を合わせた。
- GitHub checks [37722308221](https://github.com/disnana/Nagi/actions/runs/37722308221)とwebsite [37722307935](https://github.com/disnana/Nagi/actions/runs/37722307935)はこのmainで成功。Linux fullとLinux/Windows/macOS ARM/macOS Intelのpackage/native jobのstep成功を読み戻した。正式release stepはskipであり、公開成功には数えない。
- 今回新たなruntime試験・脆弱性再現・負荷試験・外部攻撃は実行していない。CIの既存試験成功は0.2.0機能の保証ではない。source hashとjob IDは[baseline.json](baseline.json)に固定する。

## 現状・根拠・不足

行番号は上記mainのもの。関連テストの存在と、すべての入力に対する保証を区別する。

| 境界 | 現在の実装・契約と一次根拠 | 0.2.0で解決する不足 |
|---|---|---|
| Principal/Grant | [auth.rs](../../../runtime/src/auth.rs):27,55,63,84、[登録](../../../compiler/src/stdlib.rs):611,631。private field、非Copy/Clone/Serde/shared、nominal permission。Rust issuerはpublicでtrusted | credential verifier/policyの正しさ、expiry/revocation、requestの有効期間は未実装。Rust factoryがprivateなNagi constructorと同じ保証だとは説明しない |
| Auth Scope | [既存計画](../compiler-rust-boundary-plan.md):163–180。対象の実値を持つ`Grant<Permission, Scope>`は将来方向。調査開始時の旧計画では既存Grant置換・arity変更を承認待ちとしていた。request region/effectは追加しない | 最新指示後は[安全性優先の判断](decisions-and-migration.md)が実装基準。単一request-bound Grantへ置換し、旧owned delegationは0.2.0で廃止。現行mainの受理とは区別 |
| route | [routes.rs](../../../compiler/src/routes.rs):4,26,54、[HTTP](../../../runtime/src/http_server.rs):596,610,716。legacy decoratorと標準Appは別経路。route/route_mappedの引数にauth policyなし | 全登録経路・dynamic path・HEAD fallback・OPTIONS・404/405・組込routeのpolicy網羅。関数名から認可を推測しない |
| credentials/header | [HTTP](../../../runtime/src/http_server.rs):215–238。単一headerは重複拒否、UTF-8検査。trailerはbody収集で認証headerにしない | Cookie解析、credential sourceの混在/優先順位、proxyからの外部origin/peerの信頼契約 |
| response/XSS | [HTTP](../../../runtime/src/http_server.rs):422–466。textはplain、htmlは文字列をそのままHTML bodyへcopy。header name/valueとframingはnative検証。legacy Htmlも生文字列 | 文脈別HTML生成、URL attribute、active content、CSP。htmlという型名だけではXSS防御にならない。raw APIの存在自体を確認済み脆弱性とは扱わない |
| Cookie/Session/CSRF/CORS | HTTP responseは複数Set-Cookieを保持。[tests](../../../runtime/src/http_server/tests.rs):426,840。専用std API/専用契約は確認できない | cookie属性・同名曖昧性・session発行/rotation/失効・CSRF token/origin・CORS preflight。任意header機能で手書きできることは標準保証ではない |
| SQL | [旧Db](../../../runtime/src/database.rs):120,130,146,157。query等はbind、execはbatch。新SQLite [session](../../../runtime/src/sqlite/session.rs):54,663–737は常設authorizer、prepare、一文、anonymous bind/shape検査。dynamic SQLの文字列も受理 | Parametersは値とSQL構造を分けるが、動的なSQL構造そのものの信頼性やtenant policyを証明しない。schema preflightはopt-in/literal、SQL実行・値型/NULL検査ではない |
| SSRF | [DESIGN](../../../DESIGN.md):15、[HTTP設計](../../library-design.md)。標準送信HTTPなし。Rustで独自clientを追加できる | URL allowlistだけでなく実接続先/DNS/redirect/proxy/TLS/再利用connectionを検証できるclient境界 |
| HTTP DoS | [Options](../../../runtime/src/http_server.rs):481–581、[dispatch](../../../runtime/src/http_server.rs):716–819、[serve](../../../runtime/src/http_server.rs):907–1005。body/header/capacity/各期限、panic応答とjoin。request permitはbody/handlerを覆うが認証専用予算なし | auth/crypto/session/DNS workと各cache/key数・response生成の予算。non-yielding/既受理DB副作用/全OOM/DDoSの回復は現行でも保証外 |
| frontend DoS | [parser](../../../compiler/src/parser.rs):4–30,171,450,666、[source](../../../compiler/src/source.rs):383–394。user source 2 MB、import合計8 MB、depth64/files128、内部generated Low64 MB、type/block64・expression128 | 新security metadata/planでも有限上限を維持。保存Lowはuser source扱いであり、内部64 MBを公開入力へ拡張しない |
| High/Low/Rust | [pipeline](../compiler-pipeline.md):5–38、[ADR006](../adr/006-sealed-codegen-input.md)、[capabilities](../../../compiler/src/capabilities.rs):7–88。Low再check、canonical ID、sealed plan、payload/phantom/callback区別 | 新security resource/operationも同じ経路へ。`@replace`後の最終検査やLow metadataでpolicy factsを偽造させない |
| Task/Tx | [ADR012](../adr/012-task-result-handles.md)、[ADR010](../adr/010-sqlite-transaction-boundary.md)、[SQLite Tx](../../../compiler/src/stdlib/sqlite.rs):28–54。業務Err/故障分離、actual join、同taskのTx | AuthScope失効とTask取消/actual join、DB admission/outcome/cleanupの責任を混同しない。新authを理由に旧spawn/Tx意味論を変えない |
| build/editor | [SECURITY](../../../SECURITY.md):21–29、[ADR007](../adr/007-build-generations.md):31–33、[VS Code](../../../editors/vscode-nagi/src/extension.js):262,332、[JetBrains](../../../editors/jetbrains-nagi/src/main/java/com/disnana/nagi/NagiCompilerAction.java):45。trust gate、generation/output保護 | trust gate/OS lockはsandboxではない。手書きRust/Cargo/build scriptを含むアプリ作者をtrustedとする。JetBrainsのuntrusted否定試験の網羅性は未確認 |

## 既存検証の読み方

- [auth_boundaries](../../../compiler/tests/auth_boundaries.rs):39–82,203–307にはHigh/Low拒否、handwritten Low、native High/saved Lowの正例がある。間違ったpermission、再利用、JSON/shared/field偽造、phantom/callbackを検査する。requestのexpiryを検査する試験ではない。
- [auth sample](../../../test-nagi-code/application-examples/auth-boundary/README.md)は固定credentialで、JWS実装ではない。[smoke](../../../test-nagi-code/application-examples/auth-boundary/smoke.py):14–24,65–97と[runner](../../../scripts/verify_application_examples.py):104–124は実HTTPを確認するが、全routeの認可証明ではない。
- [HTTP tests](../../../runtime/src/http_server/tests.rs):222,426,840,881,985,1124,1154,1256と[panic tests](../../../runtime/src/http_server/panic_tests.rs)は重複header・framing・body/deadline・capacity解放・停止を検査する。Cookie policyやブラウザーの挙動を証明しない。
- [SQL tests](../../../compiler/tests/sql_check.rs)、[SQLite public](../../../compiler/tests/sqlite_public.rs)、[native SQLite](../../../runtime/src/sqlite/tests.rs)、[public consumer](../../../runtime/tests/sqlite_public_api.rs)を維持する。statement Err、rollback完了、返信喪失、Outcome::Unknownは別oracle。
- [stdlib imports](../../../compiler/tests/stdlib_imports.rs):382,446はLow metadataのstandard ID/path/capability偽造拒否。[output safety](../../../compiler/tests/output_safety.rs)と[build generations](../../../compiler/tests/build_generations.rs)はoutput/旧exe保護で、アプリの権限隔離ではない。
- [CI](../../../.github/workflows/ci.yml)のLinux fullと4 OSにはnative E2E・配布外実行・editor検査がある。専用の依存advisory gateやブラウザーmatrixは確認したworkflow内にない。GitHub側のdynamic scanningが別途ある可能性を否定しない。

## 仕様の欠陥と未実装を分ける

以上の未実装や明記されたtrusted境界を、それだけで脆弱性と呼ばない。静的解析でappのpolicy論理、全Rust依存、実browser/proxy/DNS設定、実配布物の全経路は確認していない。新機能の実現性・依存crate・最終signature・cache数値は実装前比較が必要。

一部の既存roadmap/DESIGN/open questionsには#99の公開APIをprivate/draft/CI pendingとする過去記述が残る。調査は現在sourceとGitHubを正にし、本RFCの入口に関係するroadmap/DESIGNの現状説明を同期する。保存artifactと履歴は書き換えない。

外部指針はOWASPのauth/authorization/CSRF/XSS/SQLi/SSRF/session/DoS、WHATWG Fetch、RFC 6265/9110/8725を取得し参照した。[取得URL・hash](reference-retrieval.json)。それらの一般指針をNagiの実装済み保証や認証済み評価に読み替えない。
