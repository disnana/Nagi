# SF05削除・移行inventory

baseline main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`。機械探索のsource SHA-256は[legacy-inventory.json](../../../benchmarks/results/security-sf05-validation-2026-10-09/legacy-inventory.json)。履歴artifactは編集対象外。検索hitは削除対象の証明ではなく、user shadow・FailureKind::Sql・trusted hostを個別に分類する。

| 層 | 旧入口/対象 | SF05の責務 |
|---|---|---|
| compiler builtin/type | check.rsのDb/db_*、checked.rsのDb生成先、emit.rsのDb/Sql呼出し | checker-only migration tombstoneを先に拒否し、実行/生成分岐を削除。ユーザー同名関数/classのcanonical identityを維持 |
| canonical standard | stdlib.rs/sqlite.rs、check/sqlite.rs、checked/emitの動的SQLmaterialization | Query resource/literal operation、constructorのsealed literal payload、Query必須の3操作 |
| runtime export | lib.rs/database.rsのDb/Sql/old worker、sqliteのpublic Sql引数 | Db/Sqlをpublic標準から削除。FromRow/indicesは維持、private test用動的SQLは標準factoryにしない |
| runtime lifecycle | sqlite session/adapter/authorizer/Options/Outcome | Query移行による契約変更なし。trusted reservationはnative容量待ちと同期enqueueを結び、Grant.submitの一回permitを利用 |
| opt-in SQL | sql_check/collect.rs/engine.rsとsql_check.rs | canonical literal Queryを収集。通常checkのengine非依存を維持、未知Query構造とParameters bind数をruntime保証に区別 |
| compiler positive/negative | sqlite_public、owned/nullable_database、builtin_type_contracts、class_field_types、ownership_calls、list inference、graph/modules/scoped_tasks、resource contract fixtures/inventory | 同等の型/所有権/NULL/row/native観測をPool/Tx/Queryへ移す。旧入口migration負例はparse/resolve成功と元位置を保つ。SF01 serve負例でDb拒否を誤って目的GREENにしない |
| runtime native/test consumers | sqlite public tests/API、database旧tests、actor lifecycle、concurrency bench | Query移行と削除後native parity。旧multi-statement execはtrusted fixture bootstrapへ、request標準へ再exportしない |
| application/examples | CRUD/inventory/device-settings/result-api/web-demo、sqlite benchmarks、release verifier、HTTP capacity tools | explicit Options、Tx、literal Query/Parameters、Failure変換、trusted bootstrap。HTTP handlerの業務Err/cleanup契約を維持 |
| Docs/tools | DESIGN、ADR、日英database/sqlite/sql-check/builtins/security/migration、IDE/resources、website/examples verifier | 旧APIを推奨しない、未リリースと保証限界を明示。歴史的0.1.x migration情報とartifactは保持 |

## 初期RED

契約固定後の3件は旧Db型・旧db_open・canonical sqlite.exec string入力がcheckerで受理されるためRED。全fixtureのparse/resolveは成功し、panicはtestのexpect_errであり処理系panicを目的証拠にしていない。新constructorの未定義失敗を目的REDへ数えていない。[原ログ/command/exit](../../../benchmarks/results/security-sf05-validation-2026-10-09/initial-red.json)。初回ログはHigh段階で停止した。追加の[全経路RED](../../../benchmarks/results/security-sf05-validation-2026-10-09/initial-red-all-paths.json)は各testでHigh・元Highを削除した保存Low・手書きLowのparse/resolve成功後の旧API受理を記録し、全9経路がRED。実装後は目的診断と全3経路のsource行をassertする。


## 独立期待とconsumerの最終分類

resource_contract_characterizationの手書き期待は全registryの39資源・83操作を比較し、SQLite内は9資源・19操作。QueryのCopy/storage/shared/Debug可、Serde/equality不可とliteralのMove入力、query/all/execのReference/Move/Moveを独立に固定する。literalのpayload/indexを改変したsealed planはchecked_testsで拒否する。既存Failure accessor/enum/constantsの集合完全性も保持する。

| consumer | 移行後の観測／残す境界 |
|---|---|
| sqlite_public + security_sf05_native | High・High削除後保存Low・手書きLowで実Rust/native。評価順、Err/panic、NULL/数値/bytes、Txのimplicit loan、Query選択/返却/Copy/shared、protected SQL predicateと実HTTP denial |
| owned_database / nullable_database | trusted manual FromRowをhostに残す。標準generic行型のscalar/opaque資源制限を緩めず、旧同等値/NULL/列shapeを検査 |
| builtin_type_contracts / class_field_types / scoped_tasks | 旧Dbの削除を別row/field/Task負例の成功に流用せず、Pool/Queryと従来の非DB fixtureに置換。unsupported scalar rowの元行は維持 |
| graph / modules / list ownership | canonical SQLite操作をDB nodeとして分類。ユーザーDb/db_all/literal等の名前を占有しない。通常Result/owned推論は独立fixtureで維持 |
| runtime sqlite / public_api / rustdoc | public Queryのみ。private test用Sql::Ownedとfault/gate fixtureを公開しない。旧database worker/testsを削除、FromRow/indicesは現行decoder専用に維持 |
| application / tutorial | 固定DDL/seedはmainのHTTP登録前だけに呼ぶtrusted startup管理。handlerは固定QueryとParameters。INSERT/UPDATE RETURNINGは同Txのexec＋id/last_insert_rowid readonly queryへ移行 |
| editor / release verification / cost | 旧builtin completionを削除。配布外checkはCargo/runtimeなしで診断。生成/手書き費用は同じQuery/Options/close条件で小さい既存測定だけ行う |

compiler/src/modules.rsのDb builtin spellingとcheck.rsのDb/db_*は、旧標準名を元位置付き移行診断まで到達させるtombstone。runtimeのSqlはsqlite/session private、Owned variantはcfg(test)のみ。FailureKind::Sql、rusqlite set_db_config、ユーザーdb_all fixture、actorの歴史的Db Dropコメントは旧実行入口ではない。過去の設計/測定は先頭のSF05 superseding注記で区別し、当時の原結果を改変しない。

最終検証と未確認範囲は[結果](../security-foundation-sf05-results.md)。zero filter/skip、処理系panic、別API未定義、ネットワーク失敗を目的GREENに数えない。
