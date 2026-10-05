# ADR 007: ビルドのアプリIDと成功世代を分ける

状態: Phase 2の採用設計。Q-001の承認範囲で進める。実装前の記録であり、検証結果は[進捗](../progress.md)へ追記する。

## 問題

現在のアプリIDはcanonical sourceと論理outの組から決まる。別アプリのcache上の実行ファイルは分かれたが、同じアプリの再buildは同じファイルを上書きする。Cargoのtarget lockはNagiのpublish/runまでを保護しない。同じoutの互換Low/Rust/manifestも、Cargoだけでは調停できない。

## 採用する境界

初回checkと非書込みのinput保護が成功した後、最初の生成書込みの前にcanonical out単位のOS lockを取得する。待機中に出力が変わり得るため、取得後にもinput保護を確認する。parse/check前にoutを作らない既存負例は維持する。入力読込み全体や外部ファイルの同時編集を原子的snapshotにする契約は追加しない。

常設lock fileはread/write・非truncateで開き、`std::fs::File::lock`を使う。unlinkしない。再入・複製handle・独自の複数lockは持ち込まない。lock失敗を無視しない。生成→Cargo→publish→latest更新まで保持し、runの起動前に解放する。同じoutのcheck/lower/cost書込みもこの境界を使う。symbols、map、非書込みeditor checkは対象外。

同じhandleで先に`try_lock`する。成功時はそのlockを保持し、重ねて`lock`しない。競合時だけ標準エラーへ`waiting for output lock: <path>`を出し、`lock`で待機する。別のlockや独自のpolling待機は追加しない。待機中であることを利用者へ示すとともに、回帰テストはこの通知をbarrierとして使う。一定時間markerが現れないことだけを、直列化の成功根拠にしない。

package/app IDは維持し、generationは毎回新しく作る。generation namespaceはappごとに分ける。同じ親内の新しいstagingへ生成Low/Rust/manifest、読み取り済みsource/provenance、引き継いだCargo.lockを保存する。通常outの互換ファイルは引き続き作り、Cargo不在時にもmanifestを調べられる現在の用途を残す。runtimeへの相対pathは、それぞれのmanifest所在から計算する。

generation manifestはstable package名、`autobins=false`、世代固有の`[[bin]]`を持つ。Cargoもそのbinを指定してbuildする。親nagicが終了してCargoだけ残る場合でも、孤児Cargoが次世代のcache上のexeを上書きできないようにする。packageまで世代名へ変える方式は採らない。Cargo.lockのpackage identityと依存cacheを保つためである。

Cargo成功後、exeをコピーし、stagingを新しい公開generationへrenameする。可変cache/projectionとのhard linkは使わない。互換出力の更新が済んでから、同じfilesystem内の新しいmetadata fileを閉じ、renameでlatestを置き換える。先に旧latestを消す処理は入れない。旧成功世代は上書き・削除・killしない。

失敗時は旧latestを維持し、新世代をrunしない。互換ファイルが途中まで更新された場合はその事実を診断し、全ファイルが旧版へ戻ったとは説明しない。runはlock内で確定した成功世代のpathを保持し、解放後にlatestを再読込みして起動対象を変えない。`native:`は実際の公開exeを示す。scriptsもcache上のpackage名からの推測をやめ、成功metadataまたは`native:`を使う。

## 固定しない範囲

外部Rust、path crate、runtimeの参照先全体はsnapshotしない。任意Rustのmodule/includeを自動でコピーする仕組みは作らない。協調しない外部Cargo、悪意あるdirectory差し替え、OS crashへの耐久性、複数互換ファイル全体のatomic置換は保証しない。advisory lockはsandboxではない。

## テストの変更と観測

Q-001に従い、「再buildで同じexe path」という内部期待を「同じapp ID、異なる成功generation」へ更新する。stdout、別app分離、project cwd、input保護、lock継承、旧exeのbytes・動作は維持する。

先に、動作中旧exeと新build、同じoutの同一/別app・等価path、既定/明示cache、writing check/lower/cost、孤児Cargo、Cargo失敗、projection失敗、latest置換失敗、完全なmetadataの並行読込みを観測する。barrierと期限を使い、単にsleep後の偶然の順番を成功根拠にしない。実装後は全suite、4 OS/package/editor、conformance/fuzz、旧版とのcold/warm build比較を行う。

競合テストは、先行Cargoの停止barrier、別handleの`try_lock`が競合を返すこと、後続writerの待機通知、解放後の終了・stdout・snapshotを分けて観測する。入力保護の再検査は待機通知を受けてからprojectionをinputへのhard linkに変え、解放後の拒否とinput保持を確認する。補助wrapperとCargoにも期限・kill/waitを設ける。native holdと孤児wrapperの終了条件を混同しない。

## 一次資料と採否

- [Rust File::lock](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock): 1.89以降の標準APIでOS間のlockを扱う。handleを閉じて解放し、再入の未定義な挙動に依存しない。Nagiの開発/CIはstableで、新しいMSRV契約は加えない。
- [Rust rename](https://doc.rust-lang.org/std/fs/fn.rename.html): 同じfilesystem内でfileを置き換える。Windowsを含む実テストで確認し、失敗時に削除して再試行する方式は採らない。電源断後の耐久性までこのAPIから推測しない。
- [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html): packageとbin名を分ける。世代別binでcache中間物も分離し、既存package identityは維持する。

新依存、unsafe、全面rewriteは不要な案である。4 OS実証のためにそれらが必要になった場合はStopする。
