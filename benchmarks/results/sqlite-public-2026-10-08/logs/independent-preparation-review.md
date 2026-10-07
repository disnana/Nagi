# SQLite 公開準備の独立レビュー

対象: `/workspace/Nagi-sqlite-public`、base `676576724829e45b077b58628bfe2417e6cf3673`、commit `8cd703f` とレビュー時の未commit CI/doc差分。AGENTS.mdを最初に読み、コード・Docs・依存・lockは編集していない。レビュー対象sourceのhashと全元行は `independent-preparation-review-snapshot.json` と `independent-preparation-review-original-source.txt` に保存した。後続の修正は本レビューの対象外。

レビュー開始時はcapacity限定vendor patchとALLOCATIONが未承認だった。終了時の親agent通知では、ユーザーは公開SQLiteに必要な依存置換/API・Failure変更も含め長期設計を優先する作業を承認した。本メモは選択材料の評価であり、vendor採用・実装・完成を示さない。

## 指摘

**P2: rendered diagnostic の元source行/file名が checker の失敗種別判定を汚染する。**

- `compiler/examples/sqlite-contract-check.rs:28` は `diagnostic.contains(fragment)` で意図した失敗を識別するが、同file:50でそのdiagnosticへ `loaded.diagnostic(&error)` を渡す。
- `compiler/src/source.rs:130`、:246はエラーメッセージにファイル名と元source行を付ける。source行はcheckerの失敗理由ではない。
- `compiler/tests/fixtures/sqlite-contract/matrix.json:201` の06-copy-txと:639/:676/:715のnested copyはfragment `copy` が元行自身に含まれる。:814の `json_encode` と:534の Future returnにも同じ問題がある。
- 実測した通常サイズ反例は `ordinary-copy-wrong-error.nagi` の `duplicate = copy(view(value))`（valueはi64）。生checker Errは `viewにはstr/bytes/Listの所有値が必要です` で非Copy資源の拒否とは異なる。しかしobserveをそのまま使うと、line=3を維持したrendered diagnostic内のsource行 `copy(...)` により `matches_contract("fail", ..., "copy", ..., Some(3))` がtrueになる。証拠は `independent-oracle-regression.rs` / `.log`。
- 既存の3 oracleは誤診断の文字列を手で渡すだけなので、このobserve→render→predicate経路のfalse GREENを検知しない。CIに同oracleを加えても現在はこの欠陥が残る。
- 必要な対策: 機械判定には生checker error、表示にはrendered diagnosticを別に保持し、実際のobserve経路で「source/file名だけにfragmentがある同じ元行の無関係Err」をREDにする回帰を加える。fragmentは原則として失敗理由を識別できる語にする。元source行・pathの照合は維持する。一般的な `copy` などのfragmentだけでは将来の全失敗理由を厳密に区別する保証まではない。

親agent依頼による追加照合: `compiler/examples/task-contract-red.rs:32/:62` にも同じpatternがある。既存148入力のうちspawn-outside-scopeでは期待fragment `scope` がfilenameにあり、task-parameter/returnでは `Task` が元source行にある。同種のoracle修正をTaskにも行う根拠がある。現行148入力を同じcheckerで再検査し、rendered判定と生error判定はともに148/148成功した（`independent-task-predicate-review.rs` / `.log`）。したがって既存期待を緩めず、Task semanticsを変えずに判定境界を修正できる。

現時点の保存SQLite観測は46/46がresolve拒否、matched=0。上記欠陥で既にSQLite公開contractがGREENになっているという報告ではない。

## capacity 研究の評価

`docs/internal/sqlite-capacity-research-2026-10-08.md` の一次sourceからの主要結論と、sqlite-capacity-decision.md / sqlite-public-slice-plan.md / sqlite-pool-proposal.md / ADR010との間に、選択を止める具体的な誤りは見つからなかった。

- deadpool releaseのBuildErrorはNoRuntimeSpecifiedのみ。stockのVecDeque::with_capacity、resizeのreserve_exact、private ObjectInnerという境界をsourceで確認した。0build→resizeやWorkerHandle/public Objectのsizeofで解決済みとは扱っていない。
- 提案のLayout::arrayをcrate内部の実ObjectInnerへ適用し、その型のtry_reserve_exact成功queueを同じPoolへ渡す構造は、private配置のコピー・試し確保の破棄を避ける。固定Rust sourceの空queue予約/RawVec/Layoutと整合する。擬似diffは未適用・未buildであり、実コードの正当性は保証していない。
- Optionsの非確保validationとopenの実slot予約、lazy native起動を区別する。正数/usize/Semaphore/実slot Layout、およびbusy_msのi32境界は恣意的なcapではなく既存safe APIに由来する。rusqlite busy_timeoutの範囲外expectとTokio permit上限・mpscの初期blockもsource照合した。0ms成功やpath契約を勝手に変えていない。
- ALLOCATION候補のNOT_APPLICABLE/retired=falseはslot予約時点にTx/native workerがないことと整合する。環境依存の予約失敗をINVALIDやWORKERに紛れ込ませない判断材料になる。新kindを採る最終設計は別途固定する。Arc/String/thread/Tokio内部、retain等の確保は残り、全OOM回復保証はしない。
- bb8のu32 max_size、lazy VecDeque、owned checkout、checkout検査込みtimeout、背景connectとtask-local不伝播、公開close/drain不在はsourceに沿う。mobcのlazy Vec/owned checkout、caller内connect、ready/check込みtimeout、Dropの背景回収、max_idle(0)のunlimited、公開close/drain不足もsourceに沿う。いずれもcapacity問題だけを理由に無条件置換する資料ではなく、取得予算・終了・取消・native joinを再設計/再検証する費用がある。async-resourceを優先候補にしない根拠もその管理thread/独自executor、Resolve timer FIXME等に限定されている。
- deadpool/bb8/mobc/async-resourceの4 archive SHA-256は保存metadata checksumと一致した。deadpool pool.rs/builder.rsは保存release/cache/fixed main間で一致した。版の最新性は取得したmetadata時点の結論で、将来の最新性までは保証しない。
- 保存lock差分はbb8新規2package、mobc新規16packageを示し、scratch内で既存版を維持した再解決とfeature合成の説明に整合する。deadpoolを残して比較しているため実置換時の削除差分が別途必要という留保も適切。crate build/4OS動作成功の証拠ではない。
- runtime相対path/vendor同梱、workspace rootだけのpatch非伝播、Cargo source identity変更、license/provenance、dummy metadataでのmember/default-member差を分けている。現在のpackage.py:44–45と:82ではvendorが落ちるという指摘は正しい。runtime専用manifest/default-membersの費用が明示され、実deadpool suite成功とは主張していない。

## CI・元位置・観測の範囲

- 新runnerのstage限定によりparse/resolve/panic/acceptedはfail caseを満たさない。元lineは生errorから取得しloaded.locationで元fixture pathへ対応させ、そのpath一致を検査する。この部分には具体的な欠陥を見つけなかった。
- fixture inventoryはparseだけを行い、全46入力の登録集合、負例anchorの一意性、元行/column/token開始位置を検査する。これはpublic module受理、checkerの正しい拒否、保存Low、Rust build/runの証拠ではない。
- 保存ログの3 example oracle成功、2 inventory成功、0/46 semantic match・全46 resolve REDを読み戻した。追加の小さい通常harnessでSQLite false GREENとTask raw/rendered全148一致を観測した。新runnerをSQLite契約完成としてCI GREENに読み替えることはできない。
- capacity案の実装、allocator fault injection、vendor正規化manifestでの実crate build、生成app/独立runtime配布、native取消・join・closeの再回帰、4 OS、High→保存Lowとnative build/runは未検証。巨大確保・負荷・脆弱性再現・外部attackは実行していない。

このレビューは設計選択と検証準備への独立評価であり、公開SQLite/初心者Docsの完成判定ではない。
