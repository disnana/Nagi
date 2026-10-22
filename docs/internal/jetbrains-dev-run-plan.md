# JetBrains Development / Release Run（2026-10-22）

対象は `feature/jetbrains-dev-run`、開始点1792138。本体CLI既定Releaseと0.2.1公開作業は変更しない。標準Runとlegacy Nagi: RunはDevelopment既定、標準設定のRelease選択は従来argv・probeなし。未指定の既存設定もDevelopmentへ移行し、clone/XML/exportで明示選択を保持する。Checkは従来経路を維持する。

## 能力・cache・所有権

公開0.2.0と開発compilerはともにversion0.2.0を返す実観測があるため、semverを能力証明にしない。保存flush後の既存Backgroundableでcanonical executableを解決し、SHA-256/size/mtime/fileKeyを前後確認する。helpは5秒（hash/準備も同期限を検査）、stdout64KiB/stderr16KiB、厳密UTF-8・exit0・stderr空・既知構造・profile宣言行を検査する。引数へshellを介在させず、失敗時にuser programを再試行しない。

対応helpのprofile行だけが `--profile dev` を許可する。公開0.2.0のhelpは検証済み配布binary、0.1.11はtag `nagi-v0.1.11` のworkspace version/emit println/USAGEから取得した有限のnormalized help fingerprintに限りRelease fallbackする。raw helpをnoticeへ流さない。任意の未知helpをlegacyにしない。

project serviceは確認済み完了結果を最大8 identity保持し、同時active probeも最大8。in-flight共有/refcountは採用せず、同時missは各Runの独立process・期限・取消所有とする。正常cache hitは追加help processゼロ。settings/trust/project close/dispose/compiler VFS変更でepochとcacheを失効し、選択変更・実行体交換を起動前に拒否する。EDT/read actionではprobe/hash/待機を行わず、workerがprobeと観測した子processを停止・rootを回収し、子を待つ親へ短いreap猶予を置く。OS executable pathの外部置換とexecの原子性は保証しない。能力広告はconfigured executableへの既存の信頼を前提とし、任意の偽binaryを認証するものではない。

## Run content handoffの別修正

ユーザーのRun console異常ログを受け、SDK262 sourceとregistered runnerで調査。promise成功callback中に旧contentのenvironmentがdisposeされると、旧Startupがcanceledを立て、新しい受理済みdescriptorを自前disposeする経路をREDで再現した。SDK unit-test modeは通常content reuseを省くため、callbackでcurrent environmentを破棄する境界を明示的に通す。

PREPARING→ABORTED/HANDED_OFFの単一CASでterminal writerを決め、setResult前に所有移譲をclaimする。environment disposeはhandoff後に新descriptorを取り消さない。project close/disposeは別であり、setResultのSDK同期callbackが戻るまで一時process fallbackを保持して停止する。onProcessedの順序へ依存せず、受理済みdescriptorは追加disposeしない。project死でSDKcallbackがexpireした場合にprocessを残さない。ANSI/GPU/色設定やreuse無効化を混ぜない。ユーザー実機の見た目は未確認。

## 検証

`f6cb92c`の別High auditは指摘なし。独立focused56件と公開0.2.0 fallback1件も成功した。最新CI `39787558400`ではIDEA/PyCharm stableが成功し、EAP（IC263.7031.23／PC263.7031.30、Java25）は4件の取消型assertで停止した。JDK25の`CompletableFuture.reportGet`が保存したPCEを新しいCancellationExceptionのcauseへ包むためであり、SDK263の`AsyncPromise.nullizeCancelled`にも同じ互換処理がある。Architectの確認を受け、test専用`awaitFailure`で既存PCEを保持し、CEの直接causeがPCEの場合だけ一段取り出す。製品source、4件のPCE assert、root/child終了・cache0、timeoutのExecutionException期待は変えない。修正後EAPでそれらが最後まで実行されることは新HEADの必須gateとする。先行ZIP／audit／stable成功をこの完走確認へ付け替えない。

初回focusedで、incremental optionの説明中にもprofile語があるため対応helpを拒否する欠陥を発見し、独立したoption宣言行へ修正した。取消はSDK PCEがCancellationExceptionであることを保持して検証する。子processを同時にkillすると親がreapする前に孤児化し得るため、worker cleanupで親のreap猶予を設けた。元失敗logを保持。

実行logはrepository外のtracked対象外 `local-only/pr148-dev-run/`。公開0.2.0実binary、正式0.2.1 Linux CI配布候補、fake malformed/oversize/UTF8/timeout/cancel/identity交換、cache上限・再利用・独立同時miss、標準Runの保存/Stop/trust/settings/config/close/dispose/console失敗とhandoffの正負を検証する。最終結果は下記へ記録する。GUI実機、Windowsでの新候補受入と最終HEAD 4 IDE CIは別gate。


### 最終実行結果（2026-10-22）

- focused: capability/標準Run/gutter/Core/processの63件37秒成功。追加handoff/config/実公開0.2.0 fallbackを含むfocusedは35秒成功、正式0.2.1 Linux配布候補の実preparer正例は12秒成功。
- 全体266件83秒成功後、child reap待機中のworker割込みでもrootを強制終了してinterrupt flagを戻すcleanupを追加。対象18秒成功後、最終全体267件84秒成功（failure/error/skip各0）。console fallbackのactual Release表示と、受理中environment disposeでdescriptor維持・project disposeでprocess停止を含む。
- nativeは正式CI run39779857773/artifact12426778494の未公開Linux0.2.1配布候補（SHA-256 `56f7b630e639333ccf4847abd6111ece8422e1b42ab260a5da4089a2b6f3356b`）で確認。Dev/Release/warm Devはすべてexit0・出力42。同source/out、専有tmp targetへ既存release-only cacheをcopyしてCargoに再判定させた非対称条件で、速度比較ではない。Dev単発63.736秒、ReleaseはCargo表示1分42秒（harnessが後処理で失敗したためexact wallは未保存）、warm Dev単発0.301秒。warm観測だけ `--build-stats` を追加し、197 Fresh/1 compiled・full checks・Cargo実行を確認。IDEはこの観測flagを追加しない。
- 初回native harnessのlatest path誤りは、既成功Devをcold再buildせず修正した。後続harnessの「移動した管理run exeを直接実行できる」という誤期待も訂正し、移動copyの正規拒否をnegative結果として保存。元generationのRelease exeは42を出力し、Dev/Release保持copyのbytesはwarm実行後も不変。warm receiptを配布同梱の独立verifierで検証した。生成out archive・保持exe・rawlogsを保存後、自担当tmp native-targetのみ削除済み。
- tool接続が一時 `exec-server transport disconnected` となった履歴も保持し、状態不明中に二重Cargoを開始しなかった。公開CLI・元cache・他worktreeは編集/削除しない。

ZIP作成は21秒で成功（320,788 bytes、SHA-256 `e63bb107ad8c8a2867a6e5c4086ee0b55455b8fac061a9747749e40df31e6e48`）。この保持ZIPを明示指定したIC251 Plugin Verifierは10秒でCompatible、deprecated3/experimental17を報告した。警告は隠さず、API追従と最新4IDE CIを維持する。packagingのSDK region mapping通信警告はraw logに残り、build自体は成功している。CI policy160件も成功。source/testを以後凍結し、独立最終High auditへ所有権を渡す。全ログ・XML archive・native JSON/証拠はtracked外 `local-only/pr148-dev-run/`。Linux単発の結果からWindowsの時間やGUIの見た目を推測しない。最終sourceは独立High audit・exact HEAD 4IDE CI・利用者GUI受入へ渡す。
