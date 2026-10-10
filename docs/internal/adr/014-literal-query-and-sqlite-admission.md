# ADR 014: literal QueryとSQLite admission

採用設計: [SF00 D1–D3](../security-foundation/decisions-and-migration.md)。実装契約: [SF05](../security-foundation/sf05-contract.md)。公開の正式0.2.0・release/tagは未承認である。

## 判断

canonical `stdlib:std.db.sqlite` にopaque Copy Queryと直接literal constructorを追加する。標準query/all/execはQuery必須とし、任意stringをQueryへ昇格するfactoryを設けない。旧Db/db_*とpublic runtime Db/Sqlを削除する。checkerの旧入口は実行を持たない具体的migration診断だけにする。同名ユーザー定義はcanonical identityで区別する。

literal constructorのAST制限はcheckerが元位置で判断し、確定したpayloadをprivate emission planへsealする。emitterはこのplanから生成し、SQL構造や所有権を再推論しない。通常checkへSQL parser/engineを追加しない。opt-in collectorはcanonical constructorと直接Parameters builderだけを読み、SQLite engineが既存authorizerとprepare metadataでSQLを検査する。Query/Parametersの不明な構造をstatic成功と呼ばない。

Grantの所有者/対象情報はSQLの実Parametersへbindし、trusted reviewed adapterの固定owner/tenant predicateで限定する。汎用queryのGrant引数やQuery型だけでtenant保証を装わない。Txのopaque queue reservationは容量だけを予約する。Grant.submitの同期callbackで実mpsc queueへenqueueし、そのreplyをawaitする。revokeとpermitの線形化を実native admissionへ接続し、permit発行前のrevokeはenqueue0、permit発行後のrevokeは実enqueue前でも受理済みと観測する。callbackへ渡る前にgateは解放され、awaitをgateへ追加しない。

DDL/bootstrapとmanual FromRowはtrusted管理/adapter境界で維持する。request用動的factoryへ再exportしない。mutable session/permissionは同Tx predicateか再確認が必要であり、SF02の永続Session/世代をこの差分の完成保証へ含めない。

## 移行と維持する保証

Optionsは明示、begin/commit/rollbackはTx単位、SQLは匿名`?`を使う一文である。旧INSERT/UPDATE RETURNINGは同Txのexecとreadonly queryへ移し、実id/last_insert_rowidによる同等行を返す。旧複数文bootstrapはtrusted固定管理処理へ分ける。HTTPのError型が必要なdemoはprimary Errorを明示投影し、Outcome/cleanup/retiredを暗黙に捨てる変換APIは追加しない。

Pool/Txのaffine/same-task、0ms acquire、単一取得予算、native authorizer、shape/bind/NULL/row decode、業務Err、取消後cleanup、Failure/Outcome、close/actual join契約を維持する。statement Err・HTTP取消・後の失効をrollback保証へ言い換えない。

High・High削除後の保存Low・手書きLow、元位置拒否、real SQLite predicate/admission、NULL/owned/manual row、registry独立inventory、回帰と4 OSを個別に観測する。[移行inventory](../security-foundation/sf05-migration-inventory.md)と最終validation記録を参照する。
