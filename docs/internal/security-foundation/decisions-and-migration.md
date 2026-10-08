# 0.2.0: D1–D3の再評価と移行契約

[English](decisions-and-migration.en.md) · [RFC](rfc.md) · [実装計画](implementation-plan.md)

2026-10-08 JST。**実装基準・未実装**。最新のユーザー指示「正式化前に安全性と長期的な土台を優先する」に基づく。PR #100は設計PRで、現行main `62bbda9`と公開0.1.11を変更済みという意味ではない。以下の必要なbreaking changeについて、旧資料の互換性保留を理由に同じ承認を再質問しない。公開signature、native oracle、依存選定は各実装PRで具体化し、重大な追加リスクや矛盾が見つかれば記録して再評価する。

## 根拠と選択基準

現行[route/route_mapped](../../../runtime/src/http_server.rs):596はpolicyなし、[legacy serve](../../../runtime/src/lib.rs):240は別dispatcherと組込endpointを持つ。[Principal/Grant](../../../runtime/src/auth.rs):29のRust factoryとconsumeはleaseを要求せず、[旧ADR 001](../adr/001-backend-boundaries.md)はowned delegationを最小実験として許す。[raw HTML](../../../runtime/src/http_server.rs):430と[Db.exec](../../../runtime/src/database.rs):120も標準入口にある。これらは存在だけで脆弱性と断定しないが、追加された安全な入口をアプリが使わなくても実行できる設計になる。

採用基準は、標準入口の網羅、失敗時に拒否すること、同じ契約を一度だけ実装すること、有限資源、実行時の観測可能性。削除の大きさを成果にしない。Hyper/Tokio/rusqlite、既存resource registry・sealed plan・Pool/Tx・Taskは使い続ける。HTTP transportやDB schedulerの全面交換、汎用taint/region、任意Rustのsecurity解析は不要。

## D1: 単一のpolicy必須HTTP

**推奨: 旧署名を破壊し、標準App/route/serveを一つにする。** 新Appとlegacyの併存は、policyなし経路と二重dispatcher契約が残るので撤回。decorator限定保護もdynamic/Lowを網羅できない。

各method/pathへpublic/authenticated/authorizedを明示する。implicit publicはない。anonymous公開は正当なpolicyで、body/期限/CSRF/CORSを停止しない。AuthScopeを出せるhandler、Grantを要求する保護operation、mapperをcanonical metadataとsealed planで結ぶ。dynamic pathもruntime登録時にfallibleに検査する。無条件許可のアプリpolicyをcheckerが見抜く保証はない。

policyなし旧arity、旧decorator/legacy serve、unchecked register、raw app変換、旧機能を復活させるfeature flagを標準APIから除く。HEAD/OPTIONS/404/405/early errorsは同じdispatcher/finalizerを通す。`/health`/`/stream`/`/ws`の自動注入を止める。healthは必要な利用者が明示登録する。既存stream demoは有限bytes/response例へ移す。WebSocketと任意streaming APIは0.2.0標準では提供せず、旧入口を移行診断で拒否する。trusted Rust hostへ移す場合はその認証・終了契約を作者がレビューする。この機能喪失はmigration/release blocker表へ載せ、隠したlegacy経路で埋め合わせない。

HTMLはtyped fragmentを標準とし、旧Html/raw html factoryを削除する。bytes、mapper、append_headerからHTML/JS/SVGやsecurity headerを作る迂回を防ぐため、body種別とContent-Type/CORS/Session/CSP等はfinalizerが所有する。非active bytesの配信は保ち、一般的な秘密データの分類・ダウンロード内容の安全性を保証しない。

標準SQL実行も固定構造Query＋Parametersへ統一する。旧Dbとdynamic Sql/string factoryは標準から除く。Dbの利用者はPoolとexplicit Optionsへ移し、旧queue/timeout既定を無断移植しない。DDL/bootstrapは管理時のtrusted hostで実施し、request用dynamic SQL factoryへ再exportしない。literal QueryでもSQLの意味・tenant権限は自動証明しない。

## D2: AuthScopeと単一のrequest-bound Grant

**推奨: PrincipalをAuthScopeへ、Grant[P]をrequest-bound契約へ置換する。** 別名ScopedGrantと無期限Grantの二系統は作らない。Grant[P, S]による汎用target型は比較したが、今回必要なi64対象実値・private request ID/leaseで表現でき、一般payload/region体系を増やす理由がないため採らない。Pはnominal permission markerで、実payload扱いにしない。この判断は[将来方針](../compiler-rust-boundary-plan.md#auth-scopeの将来方針)のScope実値の方向を具体化し、旧arity互換性保留を0.2.0で置き換える。

AuthScopeはverified subject・credential source・request ID・期限・private leaseを保持する。Grantはそのlease、subject、対象実値、permissionを保持して一回consumeされる。subjectだけのpublic Rust factoryと無検査のparts取出しは廃止する。trusted verifierのidentity結果はそれだけでproofではない。dispatcherが生きたrequest contextへ結び付け、trusted authorizerはそのAuthScopeとレビュー対象policyの成功を使ってGrantを発行する。adapterに自由にleaseを生成/延長/再活性化させるpublic factoryは作らない。issuerの虚偽のverification/policy判断は依然trusted境界であり、型で証明したと宣伝しない。

proofは非Copy/Clone/Serde/shared/owned field、same-task。同taskの引数/returnとowned local Option/Resultは可。Task/Actor/background、wrapper/shared state、proofを捕捉したFutureの転送も拒否する。旧owned delegationはbreakingとして廃止し、非proof job inputを転送してtrusted service adapterが独立に再認可する。service-proof汎用APIやrequest→無期限proofの変換は0.2.0へ追加しない。

requestが正常終了/Err/panic/timeout/cancel/unpolled Dropしたら失効。期限はrequest上限とcredential/session絶対期限の最小値で、session idle touchやawaitで延長しない。capacity待機後、短い同一gate内で有効性/時刻を検査してoperation/対象にboundしたprivate execution permitを一回発行する。発行が局所失効との線形化点で、queue予約やFuture生成ではない。permitなしnative commandを開始しない。trusted consumer APIもこのadmission経路を必要とし、生partsだけを受けて標準保護operationを実行するAPIにしない。

gate後に受理済みのDB/外部I/Oは取消・期限で巻き戻らない。DBの権限変更やsession revoke commitとgateは同じ原子操作ではない。即時整合が必要な更新は同DB Txの権限/session世代predicateを要する。Session表と対象データは同じSQLite fileに置き、同じTxから参照できる必要がある。現行authorizerはATTACHを禁止するので、別file/別storeでの独立lookupはこの保証の代用にならない。全外部副作用・任意Rust adapterの内部がこれを守る静的証明はない。

## D3: 永続Sessionを標準にする

**推奨: 既存SQLite Pool/Txによる永続server Session＋opaque host-only cookie。** memory-onlyは再起動/多processで状態が分かれるため標準production storeにしない。stateless signed cookieはserver失効/rotationのため追加状態が必要になり、同じ保証を二重実装するため採らない。外部store公開interfaceは初版の実装に必要なく、分散要件とnative oracleを別途設計するまで追加しない。メモリfakeは非公開test-only。

- IDはOS CSPRNGの256bit値。DBにはレビュー済みhash部品で求めたID digestを保存し、raw bearer IDを保存しない。entropy/生成失敗は拒否し、秘密をDebug/Serde/logへ出さない。digestだけでDB改変・backup侵害・CSRF秘密漏洩を防ぐ保証はない。
- lookupは有効ID/世代/idle/absolute期限を確認し、有限のidle touchもTxでatomicにする。touchでabsolute期限は延ばさない。rotationは旧世代のcompare-and-swap、旧ID無効化と新ID作成を一Txでcommitする。並行rotationの成功は一つで、敗者から新cookieを送らない。
- capacityはDB内の生存session数、期限切れ/失効/rotation情報を含む保存総行数、行ごとのbytesを個別に制限する。発行/rotationのinsertとcleanupは同じTxで総行数を競合検査し、有限cleanupが追いつかず上限を超えるならinsertを拒否する。app processごとのカウンタだけで総容量を保証しない。失効情報の保持は元sessionの絶対期限までの有限期間以下とし、削除後も不在IDは拒否する。IDを意図的に再利用しない。無限audit/history表を標準storeへ持たせない。掃除のbatch/queueとanonymous pre-login sessionも有限。満杯で別の有効sessionを黙って追い出さず、発行を失敗させる。
- 上記は論理record/bytesの上限で、DB page/journal/WALの物理bytesと同一ではない。SQLite max_page_countのDB page制約とjournal_size_limitの回収目標を区別し、後者をactive WALのhard capと説明しない。physical disk上限は配置時のfilesystem quota等の運用境界で、disk full/I/O failureはfail-closed。共有DBの設定をstoreが勝手に変更せず、採用設定・長期reader/checkpoint条件・quota不足の診断をSF02で固定する。有限通常入力で設定と失敗分類を検証し、disk枯渇実験はしない。
- revoke/logoutはDB commitで旧ID/世代を無効にする。**そのcommit後に開始する新規lookupは拒否**。既にlookup済みのrequest snapshotは有限期限まで存在し、全processへ即時broadcastされたとは説明しない。local cancelはlocal gateで失効。Session表と対象データを同じSQLite fileに置き、session世代を同じTxのpredicateへ入れた保護更新だけがそのTx内の整合を持つ。別file/別storeからの事前lookupは同等でなく、ATTACHで迂回しない。
- store timeout/busy/破損/crypto failureは拒否し、401のcredential不正と503等の運用障害を区別する。接続/結果不明をmemory storeや旧IDへfallbackしない。rotation commitの応答喪失/取消はOutcomeUnknownとして再認証し、commit未確認で新cookieを送らない。受理済みTxの取消がrollback完了という契約へ変わるわけではない。
- process中の期限はmonotonic clockに基づく。永続期限には信頼するwall clockとDBに保持する非減少の観測時刻を用い、後退検出はfail-closedにする。再起動でも期限を新たに全延長しない。時計異常の復旧手順、共有file利用processの時計同期、DB file権限、SQLite durability設定をDocsへ明記する。DB backup復元はsessionを無効化する管理手順が必要で、古いbackupからrevocationを自動復元できる保証はない。共有fileの限定的なmulti-process試験は行うが、multi-node/network filesystem/distributed availabilityは対象外。
- Session ID cookieはSecure/HttpOnly/Path=/、Domainなしの__Host-、明示SameSite。standard issuance/rotation/認証状態/CSRF応答はno-storeをfinalizerが所有し、managed header/raw responseで上書きさせない。実TLS/対応browser試験をacceptanceにする。

0.1.xに標準Sessionはないため既存server Session dataの自動migrationはない。独自cookie/tokenからは再認証と新session発行へ移す。任意旧tokenを検証なしで新Sessionへ変換しない。永続storeのschema/version migrationはSF02で専用testと配布手順を作る。

## 互換性の保持範囲

| 保持するもの | 理由と範囲 |
|---|---|
| High/Low文法、import/canonical ID、sealed facts | 二つの入力から同じ検査。旧security metadataの読み替えや免除はしない |
| move/view/Result/Task/spawn/Actor/Supervisor | auth proofのtransfer禁止を除き既存意味論を変える必要がない。業務denialはResult、panic等は従来故障。HTTP停止/joinも維持 |
| Pool/Tx/Parametersと取得・busy・close/OutcomeUnknown | 既存native lifecycleは必要で、安全なSQL構造へ統一しても変更不要。旧SQL引数署名だけ移行 |
| JSON/text/non-active bytes、非reserved headers | raw active content/security policyの迂回を作らない出力として有用。DTO秘密分類は保証外 |
| typed Rust externと信頼したadapter | crypto/native資産の境界として必要。独自server/raw network/管理SQLは標準Foundation保証外と明示し、標準bypass再exportにしない |
| 既存0.1.x tag/配布物 | 過去成果を変更しない。0.2.0のunchecked互換モードとして再梱包しない |

旧HTTP/proof/raw HTML/Db/dynamic SQLのdeprecated並走、warningだけの実行許可、旧factory aliasesは**保持しない**。

## 移行対応表と検証

| 旧利用 | 0.2.0移行 | 必要な実証 |
|---|---|---|
| route/route_mapped、decorator、legacy serve | 単一Appへ明示policyと対応handler/mapperを登録 | 旧入力はcheck診断＋元行。public/protected/HEAD/OPTIONS/dynamic/mapper正常・拒否wire、同じ仕事のnative完了 |
| 組込health/stream/ws | health明示登録、streamは有限response例、WSは標準非対応を明記 | 注入なし、旧入口の拒否、必要endpointのみ動作。trusted host例は保証外を明示 |
| Principal factory、無期限Grant issuer/parts、Task delegation | AuthScope-bound issuer/Grant consumer。job inputを分け再認可 | Rust旧consumer compile-fail、生subject/proof/lease偽造拒否、permission/対象/request違い、期限/gate順序、Future転送拒否 |
| Html/string/bytes＋HTML header | typed renderer＋managed response | alias/mapper/Low/byte-header迂回拒否、browser context/出力上限、非active bytes成功 |
| Db/dynamic Sql、値をSQLへ連結 | Pool＋literal Query＋Parameters。管理SQLはtrusted bootstrap | 所有権/SQL shape/値/NULL/schema/authorizer/Tx close回帰、動的構造のcheck拒否、移行後DB結果 |
| 独自cookie/セッション | 再認証＋永続Session、明示origin/source/CSRF | duplicate/属性/TLS/no-store、並行rotate/revoke/保存総数capacity、期限切れ/rotation/revoke少数反復とcleanup遅延、restart/crash/clock/OutcomeUnknown、handler未実行とcleanup |

各行をHigh、High→生成Low→Rust、保存Low、手書きLow、native Low/replace、Rust consumer、配布外へ対応付ける。旧の拒否はparse/import失敗や未定義symbolだけで成功扱いせず、migration診断をcheckerに持たせる。動く旧API実装を残さず、診断用のtombstone identity/signature認識だけ保持してよい。元位置/別制限/panic/0件filterをrunner oracleで検査する。

実装PRごとにcontract RED→適切な層の実装→全経路native→必要な回帰/4 OS→日英Docs/DESIGN/実行sample→独立reviewを揃える。SF08で全旧入口inventoryとstandard runtime public export差分を検査し、単なる推奨warningを安全保証に数えない。compilerが任意trusted Rustや任意許可policyの妥当性を証明する範囲には広げない。merge/tag/正式releaseは承認前に実行しない。
