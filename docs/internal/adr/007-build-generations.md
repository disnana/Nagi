# ADR 007: ビルドのアプリIDと成功世代を分ける

状態: Phase 2の採用設計。Q-001の承認範囲で進める。実装前に固定した設計とレビューによる補足を記録し、検証結果は[進捗](../progress.md)へ追記する。

## 問題

現在のアプリIDはcanonical sourceと論理outの組から決まる。別アプリのcache上の実行ファイルは分かれたが、同じアプリの再buildは同じファイルを上書きする。Cargoのtarget lockはNagiのpublish/runまでを保護しない。同じoutの互換Low/Rust/manifestも、Cargoだけでは調停できない。

## 採用する境界

初回checkと非書込みのinput保護が成功した後、最初の生成書込みの前にcanonical out単位のOS lockを取得する。待機中に出力が変わり得るため、取得後にもinput保護を確認する。parse/check前にoutを作らない既存負例は維持する。入力読込み全体や外部ファイルの同時編集を原子的snapshotにする契約は追加しない。

常設lock fileはread/write・非truncateで開き、`std::fs::File::lock`を使う。unlinkしない。再入・複製handle・独自の複数lockは持ち込まない。lock失敗を無視しない。生成→Cargo→publish→latest更新まで保持し、runの起動前に解放する。同じoutのcheck/lower/cost書込みもこの境界を使う。symbols、map、非書込みeditor checkは対象外。

同じhandleで先に`try_lock`する。成功時はそのlockを保持し、重ねて`lock`しない。競合時だけ標準エラーへ`waiting for output lock: <path>`を出し、`lock`で待機する。別のlockや独自のpolling待機は追加しない。待機中であることを利用者へ示すとともに、回帰テストはこの通知をbarrierとして使う。一定時間markerが現れないことだけを、直列化の成功根拠にしない。

package/app IDは維持し、generationは毎回新しく作る。generation namespaceはappごとに分ける。同じ親内の新しいstagingへ生成Low/Rust/manifest、読み取り済みsource/provenance、引き継いだCargo.lockを保存する。通常outの互換ファイルは引き続き作り、Cargo不在時にもmanifestを調べられる現在の用途を残す。runtimeへの相対pathは、それぞれのmanifest所在から計算する。

generation manifestはstable package名、`autobins=false`、世代固有の`[[bin]]`を持つ。bin名はapp IDの16桁hashとgenerationから作り、長いsource stemを繰り返さない。Cargoもそのbinを指定してbuildする。親nagicが終了してCargoだけ残る場合でも、孤児Cargoが次世代のcache上のexeを上書きできないようにする。packageまで世代名へ変える方式は採らない。Cargo.lockのpackage identityと依存cacheを保つためである。

Cargo成功後、exeをコピーし、stagingを新しい公開generationへrenameする。可変cache/projectionとのhard linkは使わない。互換出力の更新が済んでから、同じfilesystem内の新しいmetadata fileを閉じ、renameでlatestを置き換える。先に旧latestを消す処理は入れない。旧成功世代は上書き・削除・killしない。

内部layoutは`out/.nagi/apps/{app_id}/`にまとめる。`generations/.staging-{generation}/`から`generations/{generation}/`へ同じ親・深さのまま公開し、`.latest-{generation}.tmp`から`latest.json`へ置き換える。公開時にmanifestの相対参照が変わらないことを検査する。metadataは`schema_version=1`、app/generation IDとexe/manifest/Low/Rust/sources/provenance/Cargo.lockのnamespace相対pathを保持する。sourcesは読み取り済みtext、元path、module ID、入力の由来を保存し、provenanceは生成Rustの行と既存の元位置・module・置換対象を保存する。pathは表示用の文字列に加え、`path_os`へUnixのbyte列またはWindowsのUTF-16列を保存する。既存の非UTF-8 cwdを扱うためであり、module IDの表現は変更しない。未対応のcolumnを補完しない。このlayoutを版間で固定した公開APIにはしない。

初回とlock取得後の保護では、互換出力とlatestを同じ出力集合として検査する。入力との一致だけでなく、互換出力が旧latestと同じfile identityを持つ場合も拒否する。別々に検査すると、互換出力の書込みで旧成功metadataを破壊してしまう。

失敗時は旧latestを維持し、新世代をrunしない。互換ファイルが途中まで更新された場合はその事実を診断し、全ファイルが旧版へ戻ったとは説明しない。runはlock内で確定した成功世代のpathを保持し、解放後にlatestを再読込みして起動対象を変えない。`native:`は実際の公開exeを示す。scriptsもcache上のpackage名からの推測をやめ、成功metadataまたは`native:`を使う。

script helperのlegacy cache lookupは、generation namespaceがない旧出力に限って維持する。namespaceがあるのにlatestがない、不正、または参照exeがない場合は失敗とする。初回build失敗を古いcache exeの成功へ読み替えない。

## IDE管理runの世代保持（0.1.3候補）

通常のCLI build/runは成功generationをimmutableに保ち、自動削除・世代数上限を設けない。`NAGI_RUN_RETENTION=latest`は公開CLIの設定ではなく、JetBrains候補が自分で起動するNagi runにだけ設定するopt-inである。手動CLI runと公開`nagic` 0.1.11のrunはこのmodeを使わない。IDE管理runの保持拡張はプラグイン0.1.3候補と同じPR sourceからbuildした開発compilerで検証中であり、compiler 0.1.12の一時候補は中止され、公開artifactはない。

`latest`は世代を一律に一つへ減らす意味ではない。回収前にOSのshared leaseとnative entry guardで使用中の世代を保護し、current latest、現在の入力に対応する世代、実行中の世代、失敗後のlast-good世代は保持する。新しいrunが失敗しても以前の成功世代を削除しない。compile成功でlatestになっただけではnative run成功とみなさず、latestに整合するrun-success recordがなければ旧成功世代を回収しない。managedなready generationに入力metadataが欠けていれば依存なしと解釈せずsweepを止めて保持し、X tombstoneの部分削除generationは外部journalで回収を再開できる。unknown entry、symbolic link、Windows reparse point、failed stagingは保留する。

成功してpublishされた全generationは`generation-inputs.json`にcanonical namespace参照と入力file identityを記録する。通常buildのgenerationも対象であり、回収は同一・別appの残存snapshotとnon-latestで実行中のrunが必要とするgenerationを保護する。入力identityが外部hardlink経由でrun leaseと一致する場合は、そのleaseとgenerationをtombstone化しない。依存ownerが消えた後も順序依存を避けるため、参照先が追加のsuccessful sweepまで残ることがある。これは厳密な総generation数・disk容量上限を設けない。依存を共有するCargo target cache（`build/native-target/`または`NAGI_NATIVE_TARGET_DIR`）はこの回収の対象外である。

cleanup journalはgeneration treeの外、`out/.nagi/apps/{app}/run-retention/{generation}.lease`に置く。private recordのheaderは`R:NAGI-RUN-2:<app-id>:<generation-id>`から`X:NAGI-RUN-2:<app-id>:<generation-id>`へ遷移し、次回のIDE管理runは正確に照合した外部X recordとexclusive leaseを使って、generation metadataが部分削除された状態から回収を再開できる。headerの未知形式や許容サイズを超えるrecordは保持し、他の正常recordの回収は続ける。journalから未確認のpathを削除対象として推測しない。このpath・headerは実装用のprivate layoutであり、公開APIとして固定しない。通常CLIにcleanup commandや自動pruneは追加しない。回収が安全と確認できない場合は余分なfileを残す。

入力にはcompilerが既知のRust path依存の`Cargo.toml`も含め、そのcrate rootが所属するgeneration全体を保護する。Cargoの任意source layout、module/include、build script、transitive dependencyはNagiで重複解析しない。別outのbuild/runへmanaged generationを入力として渡す場合は、所有generationのshared leaseと正確なR headerの下で、外部journal横へ永続的な`{generation}.exported` recordを作る。GCはexclusive取得後にもその存在を確認する。この世代は通常CLIのimmutable artifactと同様に自動回収対象から外れる。export先の所有者を推測するglobal GCや期限は加えない。leaseに外部hardlinkが残る間もtombstone化しない。別out exportやaliasのため余分に残る世代は、厳密な容量上限の保証外である。

## 固定しない範囲

外部Rust、path crate、runtimeの参照先全体はsnapshotしない。任意Rustのmodule/includeを自動でコピーする仕組みは作らない。協調しない外部Cargo、悪意あるdirectory差し替え、OS crashへの耐久性、複数互換ファイル全体のatomic置換は保証しない。retention journalの電源断後回復は実験・検証しておらず、次回回収は電源断後の耐久性を保証しない。すべてのfilesystemに対するfsync耐久やOOM時のcleanup完了も保証しない。advisory lockはsandboxではない。

## テストの変更と観測

Q-001に従い、「再buildで同じexe path」という内部期待を「同じapp ID、異なる成功generation」へ更新する。stdout、別app分離、project cwd、input保護、lock継承、旧exeのbytes・動作は維持する。

先に、動作中旧exeと新build、同じoutの同一/別app・等価path、既定/明示cache、writing check/lower/cost、孤児Cargo、Cargo失敗、projection失敗、latest置換失敗、完全なmetadataの並行読込みを観測する。barrierと期限を使い、単にsleep後の偶然の順番を成功根拠にしない。実装後は全suite、4 OS/package/editor、conformance/fuzz、旧版とのcold/warm build比較を行う。

競合テストは、先行Cargoの停止barrier、別handleの`try_lock`が競合を返すこと、後続writerの待機通知、解放後の終了・stdout・snapshotを分けて観測する。入力保護の再検査は待機通知を受けてからprojectionをinputへのhard linkに変え、解放後の拒否とinput保持を確認する。補助wrapperとCargoにも期限・kill/waitを設ける。native holdと孤児wrapperの終了条件を混同しない。

## 一次資料と採否

- [Rust File::lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock): 1.89以降の標準APIでOS間のlockを扱う。handleを閉じて解放し、再入の未定義な挙動に依存しない。Nagiの開発/CIはstableで、新しいMSRV契約は加えない。
- [Rust rename](https://doc.rust-lang.org/std/fs/fn.rename.html): 同じfilesystem内でfileを置き換える。Windowsを含む実テストで確認し、失敗時に削除して再試行する方式は採らない。電源断後の耐久性までこのAPIから推測しない。
- [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html): packageとbin名を分ける。世代別binでcache中間物も分離し、既存package identityは維持する。

新依存、unsafe、全面rewriteは不要な案である。4 OS実証のためにそれらが必要になった場合はStopする。
