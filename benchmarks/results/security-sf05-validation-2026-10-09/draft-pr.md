タイトル案: SF05: literal QueryとSQLite認可admissionへ統一する

## 変更

標準SQLiteのSQL構造をcanonical `std.db.sqlite.literal`の直接literalとopaque Queryへ固定し、query/all/execにQueryを必須としました。変数/連結/format/関数返却値からのconstructorをcheckerで元source行付き拒否し、ユーザー同名定義は標準APIと区別します。旧Db/db_*とruntime public Db/Sqlを実行経路ごと削除し、具体migration診断・日英Docs・例を揃えました。

trusted reviewed adapterが実Grant subject/targetを同Tx owner/id predicateへbindし、SF01の一回permit発行を実SQLite bounded queueの同期enqueueへ接続します。発行前失効はenqueue0、発行後失効は受理済みとして区別します。既存authorizer/one-statement/bind/row/NULL/Failure/Outcome/cancel/cleanup/actual closeを維持し、固定bootstrapをrequest handlerから分離しました。Queryだけでtenant制約を保証するAPIは追加しません。

## 検証

- 保存workspace exit0: raw1011pass/failed0/既存measurement1ignored。compiler/runtime production SHAは最終sourceと一致し、後差分test2filesはdefault/no-engine各12群・SQLite79/public1で検証。
- 最終native9群、High・元High削除保存Low・手書きLowの原source/生成Rust/build/run216files、実SQLite predicateと実HTTP denial。registry独立期待39resources/83operations、SQLite9/19。
- fmt/clippy/fuzz exit0、Node196actual/0skip、移行10projects/19native、日英tutorial、HTTP8native/84小業務checks。Query generated/manual費用はLinux限定の4warmup/32samplesでcallerallocation/Futureサイズとactual closeを記録し、performance thresholdなし。
- website102pages、Markdown相対links2185件欠落0。raw log/input SHA/cache provenanceと先行失敗を `benchmarks/results/security-sf05-validation-2026-10-09/`へ保存。

## 残検査と範囲

独立Sol High reviewと、このSF05最新headの四OS CIは親の統合工程で実施予定です。登録だけを実行成功と数えません。manual VS Code host GUIは未実行。warm cacheを再利用しclean buildとは呼びません。partial HTTP runはcommand自体のfixture失敗を保持し、matching compiler/sourceの成功4runと後半成功4runを台帳で区別します。

SF00 #100 / SF01 #101がmainへ反映された `3b8da226`をbaseとする独立PRです。SF02永続Session/世代、SF03 CSRF/CORS等はこの変更の完了範囲に含めません。取消/後の失効による受理済みSQLのrollbackを保証しません。merge/version/tag/release/公開は実行していません。
