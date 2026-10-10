# SF05: literal Query / SQLite admission契約

2026-10-09。SF00のD1–D3とSF01を前提にした実装契約。SF02 Sessionは未実装、0.2.0は未リリース。

## 標準API

canonical module IDは`stdlib:std.db.sqlite`。`literal(sql: str) -> Query`は直接文字列literalだけを受理する。変数、view、連結、format、関数結果を渡すとcheckerがconstructorの引数の元位置で`SF05 literal Query`を診断する。ユーザーの同名literal/query関数を占有せず、標準moduleのcanonical operation IDを確認する。Queryはopaque、値の固定構造を保持するCopy/共有可能な資源であり、公開field/JSON復元/動的factoryはない。Parametersは従来通り非Copyの値bind。

`query[T](tx: view[Tx], query: Query, parameters: Parameters)`、`all[T]`、`exec`はQueryを必須とする。旧string引数は具体的な`SF05 migration`診断で拒否する。Queryをnamed local・引数・返値・レビュー済み有限の選択へ移せる。直接literal制約はconstructorだけに適用し、Query利用時へRustのstatic lifetime制約を漏らさない。checkerはSQL parserを追加しない。sealed constructor planにliteral payloadを保存し、emitterはそのplanを使う。通常checkはSQLengineを必要としない。

旧builtin Db/db_open/db_exec/db_all/db_query/db_insert/db_update/db_writeとruntime public Db/Sql/factoryを削除する。checkerは移行診断用tombstoneだけを残す。同名ユーザー関数・classはcanonicalな旧builtinと区別する。旧APIをdeprecatedやfeature flagで並走させない。FromRow/column indicesは現行SQLite decoderで必要なので維持する。

## 実行境界

Pool/Tx/Options/Parameters/Failure/Outcome、authorizer、一文prepare、bind数/値型/NULL、列shape、acquire 0ms、取消・rollback・cleanup/actual closeは既存契約を維持する。literalはSQLの意味・tenant権限を証明しない。bootstrap/schema migrationはtrusted hostのレビュー済み管理処理へ分離し、request向け動的SQLfactoryを再exportしない。標準authorizerの既存許可/拒否を無断で変更しない。

保護DB操作はtrusted reviewed adapterで、Grantの実subject/targetをowner/tenant predicateへ実bindする。汎用queryへGrantを足しただけで権限を保証するAPIは作らない。Rust host専用のopaque queue reservationは容量待ちでありadmissionではない。adapterは予約後にGrant.submitへ渡し、callbackでsubject/targetをbindして予約の同期enqueueを一回行う。enqueue済みreplyだけをawaitし、deferred enqueueをadmissionと呼ばない。reservation/Query自体はproofではなく、adapter/predicateの妥当性はtrusted境界。

native oracleは同じSQLite file内の小さなownerデータを使う。正しいsubject/targetのみ更新し、別tenant・別targetはaffected=0で元値を維持する。local revoke-before-admissionはenqueue=0、逆順はenqueue=1で受理済みと区別する。有限queue待機後の失効も検査する。Grant/permitは一回消費。受理後取消は副作用rollbackの証明ではなく、DB内session/permission変更との整合は同Tx predicateまたは再確認を要する。SF02のsession世代検証を完成したとは説明しない。

## 検証と証拠

契約→最小RED→checker/runtime実装。High・High削除後の独立保存Low・手書きLow、alias/@replace、native同等業務、元source行拒否を分ける。parse/import/未定義API/別制限/panicは目的GREENに数えない。RED原ログとsnapshot/source SHA-256、使用cache/toolchain/command/exitを保存する。既存DB/SQL/resource registryとnative consumer inventoryを移行し、独立review、fmt/clippy/full regression/fuzz/examples/日英Docsと4OS CIを確認する。merge/version/tag/releaseは別承認。
