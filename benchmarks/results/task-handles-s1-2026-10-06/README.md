# Task結果handle S1接続の保存記録

2026-10-06。Stage 1の停止後、ユーザーの再開指示でcompiler/public runtimeを接続した。基点PR #88 `5c2c8282`、main `9ba4a104`。原ログ・snapshotは成功報告とは独立した証拠であり、古いsnapshotをproductionへ戻さない。

| 保存箇所 | 観測と修正 |
|---|---|
| compiler/01-additional-red | 76入力中6一致・70未達、parser RED。期待をparse拒否へ変えない |
| compiler/02-native-first | 手書きLowのrecord/attribute/let誤記とadapter依存・ErrorKind assertを保存。fixture誤りとTask契約を区別 |
| compiler/03-failure-clone-red | Failure wrapper copyはcheck受理→Rust E0277。physical payload遍歴でcopy/shareを拒否 |
| compiler/04-spawn-alias-red | user spawnのalias/binary/field互換RED、旧HEADとの比較、contextual lookahead修正 |
| compiler/05-native-reexport-red | check受理→関連メソッドpub useのRust E0432。sealed再export可否へ修正 |
| compiler/06-raw-check-red | public raw checkerでTask義務欠落/High-Low不一致。正式synthetic root resolverでcanonical metadata接続 |
| runtime/01-public-red〜04-public-final | production移動前のcompile RED、旧16native oracle/public API/doc/cost。private二重実装は残さない |
| runtime/05-sticky-retirement-red | 故障後大量受取のO(n²)掃除。entries128!=127のnative RED→個別ticket退役 |
| verification | checker段階・元行を含む90/90契約、三構文nativeと周辺回帰・生成探索、封印facts検査 |
| reviews/compiler・runtime | 独立レビューの縮小反例、再実行ログ、source hashesと限界 |
| cost | 同じTaskScope・Tokio・入力の手書き/生成Rust。allocation、Future、時間、binary、warm依存のpackage再build原ログ |

runtimeの旧16にretirement回帰を追加して17群。fault receiveは同じ独立release harnessで8192件97.3ms→1.75ms。任意Rust Drop/panicの回復、non-yielding強制停止、外部副作用rollbackを保証しない。同期Dropはabort要求まで。

測定はLinux x86_64、current-thread Tokio、release、25 loops×7回。allocation counterは呼出しthreadのみ。手書き/生成RustのFutureはreceive416B/discard408B、allocation数とbytesは一致。時間は共有hostのばらつきを含み、ゼロコストや一般的な速度優位を主張しない。cache使用をclean buildと呼ばない。runtime単体の一括spawn/receive測定は、言語fixtureの順次spawn/receiveとは別 workload。productionとcfg(test) layoutも分ける。

最新の結果・CI・保証範囲は[接続結果](../../../docs/internal/task-handles-s1-results.md)。S2、公開SQLite、merge/release/版更新は対象外。provenance.jsonは最終sourceと保存artifactのSHA-256を記録する。
