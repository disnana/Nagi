# import completion fixtureの非同期保存境界

2026-10-23。PR #150のCI `39907278847` / job `120263454944`で、
IC EAP `263.7031.23` / JDK `25.0.4` の
`testSourceCompletionReplacesOnlyReportedTokenIncludingQualifiedStdName` がteardown中に失敗した。
`qualified.nagi`への`Files.setLastModifiedTime`が`NoSuchFileException`になり、
`AsyncableFileIOTaskExecutor`から`super.tearDown()`へ伝播していた。

## 原因の境界と最小修正

fixtureはVFS経由で保存した後にNIOで内容を読み、さらにframework teardownより先にNIOでrootを削除していた。
exact SDKの[ManagingFS](https://github.com/JetBrains/intellij-community/blob/idea/263.7031.23/platform/analysis-api/src/com/intellij/openapi/vfs/newvfs/ManagingFS.java#L82)
はVFSとPath操作の切替時にpending/in-progress IOのflushが必要と明記している。
[保存task](https://github.com/JetBrains/intellij-community/blob/idea/263.7031.23/platform/vfs-impl/src/com/intellij/openapi/vfs/impl/local/AsyncableLocalFileSystemImpl.kt#L107)
はbytes書込後のtimestamp更新まで含むため、内容の読取成功だけでは保存taskの完了を保証しない。
[framework cleanup](https://github.com/JetBrains/intellij-community/blob/idea/263.7031.23/platform/vfs-impl/src/com/intellij/openapi/vfs/impl/local/AsyncableLocalFileSystemImpl.kt#L276)
が残ったIOをflushすることも、観測したstackと整合する。強制interleavingでの再現は未実施。

既存`NagiSaveBarrier.awaitDiskWrites()`をqualified保存後のNIO読取前と、teardownのNIO削除前に呼ぶ。
サービス停止・設定/trust復元後にflushし、その時点で投入済みのIOが終わってから削除する。
失敗時は削除へ進まず、`UsefulTestCase.addSuppressedException`へ例外を渡し、
`finally`で必ず`super.tearDown()`を呼ぶ。frameworkの既存集約機構で失敗を保持する。

flushはpending taskを現在threadで実行するか実行中IOを待ち、IO失敗を再throwする。
対象保存taskにEDT callback待ちはなく、この経路でのEDT循環待ちは見つからなかった。
保証は呼出時に投入済みのtaskまでで、任意の将来callbackによる保存まで禁止するものではない。
assert、15秒deadline、並列度、productのRun/保存実装は変更しない。

## 検証

Highによる事前source/SDK調査を完了。修正はfixtureの2箇所と例外の保持だけ。
`git diff --check`は成功。修正後の対象class・save barrier test、独立最終review、
新HEADのCIは進行中で、この時点では成功としない。
前HEADではWindows/macOS ARM/Linuxの本体検証と他3 IDEが成功したが、新HEADの代わりにはしない。
Windows実GUIの#148受入とは別のtest fixture修正である。
