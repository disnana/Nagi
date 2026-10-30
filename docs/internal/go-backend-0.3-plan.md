# Nagi 0.3.0: Goバックエンド段階実装計画

状態: **段階実装中の採用計画**。初版の基点はmain `0d1884cb5a24235a440811a9f5d16be1bf5a664a`（2026-10-26）で、Nagi 0.2.2はその後 `7c92045a04519609ae287803a31ac0816c728086` として公開済みである。0.3.0開発は0.2.2の版更新・保守と分離し、Rust既定を保ったままRust以外の生成先を追加する。最小Go backend、3 OS必須gate、List/Range、明示move、payloadなしenum、共有conformance、private lifecycle kernel、cold user async、callback identity、read-only `view[str]`、user async引数のsealed capture factsとG3aの限定capture releaseまでを実装・検証した。G3aの#206はmain `d4c2b08`へマージ済みの未リリース開発sliceであり、HTTP/DB/auth/actor/task/scope、一般borrow、完全なcleanup、性能優位、本番利用可能、0.3.0の公開日を約束しない。

## 決定済みの方向

- `nagic`はRustで実装した一つのcompilerを維持する。NagiGoのような別言語・別parser・別checkerを作らない。
- High/Low構文、名前解決、型・所有権・capability検査、診断、標準操作の意味論は共有する。生成先ごとに分けるのはtarget lowering、code emitter、runtime、native adapter、依存解決と成果物作成である。
- Rustを既定backendとして維持する。選択順は明示CLI、`nagi.toml`、Rust既定値とする。
- Rust backendはRust資産、Go backendはGo資産へ接続する。選択backendに必要なnative bindingがない、または到達可能な依存が別backend専用なら、依存chainを示すbuild-time errorにする。無視、stub化、暗黙backend切替はしない。
- 同一program内でRustとGoを混在させるhybrid build、cross-backend FFI、Rust資産のGoからの利用、Go資産のRustからの利用は0.3.0初版の対象外とする。将来検討中とは記せるが、需要・安全境界・配布負担を確認するまで対応を約束しない。Python backendも対象外である。

## 現状と最初に分ける境界

現在の`CheckedProgram`は最終検査済みASTとprivate checker factsを封印する一方、seal時に`EmissionPlan`も構築する。`EmissionPlan`には`rust_types`、`RustNames`、Rust symbol path、Rust固有のvalue/view/drop実現が入り、標準library registryも`rust_path`を直接保持する。このためparser/checkerを複製する必要はないが、`emit::rust`だけをGo文字列出力へ置き換えることもできない。

段階的に次の三層へ分ける。

1. **Checked semantics**: 既存の`CheckedProgram`、最終所有権facts、型、binding identity、resource/capability、評価順、task use、source provenance。backend名・Rust名・Go名を持たない。
2. **Target-neutral lowering plan**: class/enum/function identity、式ごとのmove/borrow/copy、cleanup edge、scope/join、標準操作ID、native symbol ID、source mappingを保持する。文字列のRust pathやGo package pathは持たない。
3. **Backend plan**: `RustEmissionPlan`または`GoEmissionPlan`が、対象言語の型名、symbol path、import、runtime call、生成fileを決める。

最初のrefactorでは既存Rust出力のbyte-for-byte golden、High→生成Low→独立parser/check→finalize、手書きLowを維持する。`CheckedProgram::seal`からRust固有planを一括削除せず、意味factsを一項目ずつ抽出してRust planが同じ結果を作ることを検査する。Rust側の境界整理に合格する前にGo emitterを広げない。

今回の限定sliceでは、既存Rustのscope退出判断だけをprivateなtarget-neutral `ScopeExitPlan`へ抽出する。stable ScopeId、Legacy/TaskScope bridge、外側のerror destination、lexical bodyの退出先、正常join、body locals退役後のcancel/drainと元error伝播、同期owner teardownのabort要求までをchecker sealで保持・検証する。Rust emitterはこのplanを消費し、既存のTaskUseによる受取/放棄identityを維持する。Goは欠落・不整合factsを共有境界で拒否し、Scope/Spawn/SpawnBindのunsupported拒否を維持する。この抽出はGo scope/task、一般owner retirement、resource close、cancel/実joinの実装ではない。

## CLIとproject設定

候補名は、OS/CPU target tripleと区別するため`backend`とする。

```text
nagic check --backend go --project app
nagic build --backend go --project app
nagic run --backend go --project app
```

```toml
entry = "main.nagi"
backend = "go"

[rust]
file = "native/bridge.rs"

[go]
files = ["native/bridge.go"]

[go.dependencies]
"example.com/acme/math" = { version = "v1.2.3" }
```

優先順位は`--backend rust|go` > top-level `backend = "rust"|"go"` > `rust`。同じCLI指定の重複、未知値、空値は既存optionと同じく入力errorにする。既存projectに`backend`がない場合のpath・生成Rust・Cargo動作は変えない。

`check`、`lower`、`build`、`run`は同じbackend解決を使う。`lower`の生成Lowはbackend間で同一とし、backend指定はnative binding/support検査だけに使う。`symbols`と`assist`はprojectのbackendを読み、対象backendで到達可能なbinding不足をeditor診断へ返すが、compilerやnative codeを実行しない。IDEは独自のbackend既定を持たずcompilerの解決値を表示する。

既存`native = ["*.low"]`はportableな手書きLow宣言・実装として維持する。`[rust]`と`[go]`はtarget-native資産である。両sectionを同じprojectへ記述すること自体は、同じNagi sourceを二backendでbuildできるよう許可する。選択backendで到達可能なexternに対応bindingがない場合だけ拒否する。未使用の別backend sectionを無言でbuildへ混ぜない。

生成先、app identity、世代metadata、CAS/cache keyへbackendを入れる。既存Rust projectの既定pathは互換のため維持し、Go成果物はRustの`Cargo.toml`、`Cargo.lock`、`src/main.rs`や成功generationを上書きしない。`--out`を共用してもbackend別stagingとatomic latestを使い、失敗したGo buildが最後に成功したRust binaryを破壊しない。ただし現行のcanonical out単位のOS write lockをbackend別app IDへ縮めない。同じ`--out`のcheck/lower/build、Rust/Go buildは一つの排他境界で順番に処理し、projection・metadata・latestの混成を防ぐ。build/runは成功generationをpinしてからlockを解放し、利用者programの実行中はlockを保持しない。別backendのbuild失敗、待機process終了、projection途中失敗でも各backendの旧成功generationとlatestを不変にする。

## Native adapterと依存の契約

extern宣言のNagi署名は共有し、target bindingを分離する。既存`@rust("...")`は互換維持し、同じexternへ`@go("package.Symbol")`を併記できる形を候補とする。最終構文はparser変更前にADRへ固定する。

診断は少なくとも次を含む。

```text
selected backend go cannot provide native function hash
required by main -> create_digest -> hash
available binding: rust (native/bridge.rs)
add a Go binding or select --backend rust
```

- 到達可能性が不明、dynamic call等で安全に絞れない場合は全候補を必要とみなす。
- `[rust]`、`[go]`、extern binding、依存moduleのbackend不一致を同じtyped diagnosticで扱う。
- error位置は元Nagi/Low宣言と`nagi.toml`の双方を示す。生成Goや生成Rustの行だけをprimaryにしない。
- Go module path、version、`go.sum`、local replacementの出所をbuild receiptへ含める。cache hitだけで依存検査済みとはしない。
- 初版は`CGO_ENABLED=0`を必須にし、cgo、build tags、toolchain directive、private module認証、arbitrary replaceは明示unsupportedにする。対応を広げる場合は別のprovenance・sandbox・配布判断を行う。

Go native adapterは任意のGo値をchecked programへ差し込める一般FFIにしない。初版はowned scalar、owned string/bytes、値class、Option/Resultのgenerated ABIだけを許す。`view`、resource、task/auth tokenの受渡しは、保持・別goroutine移送・cleanupを防げる専用wrapperができるまで拒否する。Go package内のunexported fieldだけで悪意あるadapterを安全にできるとは主張せず、native adapterをRust adapterと同様のtrusted/audited boundaryとして明記する。

## 最小Go backendの表現

最初の実行sliceは次だけを受理する。

- integer/float/bool/unit、owned `str`/`bytes`
- 同期関数、local binding、if/while、明示return
- 値classとfield access
- `Option[T]`、`Result[T,E]`、`try`、比較、基本console出力
- portableな手書きLow。同じHighと独立保存Lowの両方を実行する

`Option`はzero valueだけでNone/Someを潰さないtagged generic、`Result`はOk/Err tagとpayloadを持つgenerated/runtime typeにする。`try`はpanicではなく明示分岐と早期returnへlowerする。classはfield順、評価順、copy/move判定をchecked planから受ける。GoのGCがNagi ownership checkの代わりになるとは扱わない。

unsupported syntax/APIはcheckerで受理後にGo compiler errorへ流さず、Nagi側で`backend go does not support ...`と元位置を出す。backend capability tableはversion付きで、0件実行や未知featureを成功扱いにしない。

## runtime安全性の同値条件

実行順・割当て・schedulerの完全一致は約束しない。共有対象は観測可能なNagi意味論、正常値、業務Err、panic/fault分類、所有権拒否、resource上限、取消と終了後の保証である。

- `TaskScope`の通常完了・body Err・task faultなどasync cleanupをpollできる出口は、実taskをjoinしてから外側へ結果を返す。fault時は兄弟へcancel要求を出した後にdrainする。親Futureの取消や未poll Futureの破棄など同期`Drop`相当の経路はblockしてjoinできないため、Dropはabort要求とowner移送だけを行い、runtime側のjoin ownerがtaskの実終了までresource/permitを保持する。`context.CancelFunc`を呼んだ時点やGo wrapperのGC時点をjoin・解放完了とみなさない。
- deadline/cancelは協調的であり、non-yielding Nagi処理や任意native Go関数を安全に強制停止できるとは約束しない。timeout応答や呼出し元の待機終了と、task/native処理の実終了・resource解放を別に観測する。bounded adapterまたはprocess隔離なしのnon-yielding native処理は初版support対象外にする。
- admission permitは受理前に取得し、拒否・timeout・panic・cancel・正常終了の各pathで一回だけ解放する。queue長、同時実行、message/body上限をruntimeが実際に強制する。
- `AuthScope`、`Grant`、lease owner、DB transaction、response/resource handleはGC finalizerを正しさの境界にしない。checked cleanup edgeから明示`Close`/releaseを生成し、process終了用finalizerは使用してもleak観測の補助に限定する。
- Resultの業務Err、panic、取消、deadline、compile error、infra errorを混同しない。panic recoverはrollbackや外部副作用取消を意味しない。
- DB commit、HTTP response開始、外部adapter副作用は取消で巻き戻ると約束しない。Rust runtimeと同じ公開保証・制限をtarget別テストで固定する。
- owned/value/viewのrepresentation差がnative adapterからNagi側のlifetimeを破らないことをnegative testで確認する。

## 段階と完了条件

### G0: 境界固定とRust無回帰

- `Backend`解決、target-neutral operation/resource ID、Rust固有planの所在を追加する。
- 既存Rust CLI、生成Low、生成Rust、Cargo manifest、診断、世代、cache keyが既定入力で変わらない。
- parser/checker/emitter/runtime/CIの依存graphとunsupported一覧をmachine-readableにする。

合格条件: 全workspace、High/独立Low/手書きLow、既存native/security/resource harness、4 OS L3が成功し、代表goldenが一致する。

### G1: 最小native executable

- 上記の最小Go sliceを`go build`し、実行できる。
- `--backend go`とproject設定、Rust既定、wrong-target診断、別生成先、失敗時last-good保持を実装する。
- Go toolchain version、GOOS/GOARCH、module graph、runtime hashをreceipt/cache keyへ入れる。

合格条件: Highと元Highを削除した保存Lowが同じ出力・exitを示す。wrong arity/type/move/unsupported/native mismatchがNagi元位置で失敗する。同じcanonical `--out`へ異なるsourceのRust build、Go build、check/lowerを競合させ、OS lock待機、staging公開順、run前unlock、各backendの旧成功binary/metadata保持を確認する。projection途中・target compiler・latest更新の各failure injectionでも他backendの成功世代を変えない。Windows/Linux/macOSのpackage smokeを通す。

### G2: moduleとnative adapter

- import、enum、collection、JSONとtarget別native adapterを広げる。
- dependency chain診断、module version/sum、offline/locked再build、local dependency変更のdirty判定を実装する。

合格条件: 同じportable appをRust/Goで実行するconformance driver、各backend専用adapterのpositive/negative、cache corruption、toolchain変更、異なるtargetの成果物混同を検査する。

### G3: async、Task、終了処理

- async lowering、TaskScope、timeout/cancel、bounded admission、panic/fault伝播を実装する。
- unjoined goroutine、二重release、cancel後の誤成功、last task leakを検出するharnessを追加する。

合格条件: scopeごとのactive task/permit/resource counterが試験終了時0。正常、業務Err、panic、deadline、親cancel、所有者Dropのsocket/runtime testsと長時間soakが成功する。

### G4: backend向け標準機能

- HTTP/JSON、SQLite、auth/session/CSRF/CORS、actor/supervisorを、一つずつRust契約と照合して追加する。
- API名だけを登録してruntime未接続の状態をsupport済みにしない。

合格条件: 標準moduleごとのpublic positive/negative、実socket、DB persistence/transaction、auth lease、shutdown/capacity harnessが両backendで成功する。未対応moduleは明示拒否する。

### G5: production candidate

- 4 OS/architectureの正式L3、依存監査、配布・installer、IDE、破損cache、kill/restart、負荷・soakを完了する。
- blockerなしの独立security/correctness reviewと、同じcommitのrelease artifact再検証を行う。

「production-ready」はG5まで使わない。最小binaryが動くこと、conformanceの一部が通ること、Go toolchainがcompileすることを本番品質の証拠にしない。

## conformanceの分離

現行conformanceは期待意味論に加えて`emit::rust`、`rustc`、Rust `assert_eq!`、Future pollingを含む。このrunnerをそのままGo対応と呼ばない。

1. 言語中立のcase manifestへsource、期待check成否、診断code/元位置、期待stdout/value/exit、必要capabilityを置く。
2. frontend driverはHigh、独立保存Low、手書きLowのparse/check/finalizeを一回ずつ検証する。
3. Rust driverとGo driverが同じcase manifestを各targetでbuild/runする。
4. target固有caseは理由とtargetを明示し、shared caseのskipを禁止する。
5. 実行0件、unknown capability、driver crash、期待targetのskipを失敗にする。

時刻、allocation address、map iteration、scheduler順はcross-target equalityから除外し、公開順序保証がある出力だけ比較する。float、UTF-8、integer overflow、nested Option/Result、cleanup順は専用oracleを持つ。

## benchmark計画

Go backendが動く前にGoの数値やRust比倍率を記載しない。最初にRust既定の再現可能なbaselineを保存し、G1以降に同じsource・機能・hostでGoを追加する。

計測を分ける。

| 区分 | 計測 |
|---|---|
| cold build | 空のapp出力・target cache。依存download込みと、依存取得済みを別々に測る |
| warm no-change | source/lock/toolchain不変でのbuild。検査を省略しない |
| one-function edit | public signature不変のbody変更。再生成、target compiler、linkを内訳化 |
| public API edit | caller再検査・再build範囲を確認 |
| test compile | test artifact生成まで。test実行時間を混ぜない |
| test execution | compile済みartifactの実行、case数、並列数を記録 |
| runtime | HTTP latency/throughput、DB operation、Task/actor admission。正答・上限違反0を前提に測る |

同じ物理host、電源/CPU条件、OS、filesystem、compiler commit、Rust/Go toolchain、profile、target、feature、依存lock、cache状態を記録する。warmup 2回後に原則10回、中央値・p95・MADまたはrange、wall time、CPU time、peak RSS、生成/cache bytesを保存する。Rust `--profile dev --incremental`とGo開発buildを比較し、Rust releaseとGo production flagsは別表にする。codegen時間だけでなく`nagic`起動から実行可能artifactまでを主指標にする。cache hitはtest成功の代わりにしない。

計画作成時点のexecutorでは`/usr/bin/go`がGo toolchainではなかったが、後続開発では固定した公式Go 1.27.3をCIとローカル検証へ導入した。小規模appのbuild feedback値は[最小backend記録](go-backend-0.3-minimum.md)に条件と非対称性を含めて保存している。CIでは今後も配布元とversionを固定し、自動toolchain取得を無効化して、`go version`、`go env`の必要な有限項目とmodule graphをartifactへ残す。未実装のHTTP/actor等についてGoのruntime性能値はまだ存在しない。

## change-aware CI

既存`changes.py`はDocs/JetBrains以外をfull、`levels.py`はL1/L2/L3、`gate.py`は選択jobの成功を検査する。Go workflowを追加するだけではaggregate gateがGo成功を要求しないため、selector・expected-job contract・gate・testsを一つの変更として実装する。

- parser/checker/High→Low/semantic plan/stdlib contract/conformance manifest変更: RustとGoの両backend。
- Rust emitter/runtime/Cargo/Rust adapterだけ: Rust backendと共有frontend invariants。
- Go emitter/runtime/go.mod/Go adapterだけ: Go backendに加え、Rustで実装されたcompilerのbuild/fmt/clippyと共有frontend invariants。
- Docs/example: code blockが指すtargetの実check/build/run。target不明、routing file、selector/gate/workflow、dependency/lock変更: 広い両backend coverageへfail-safe。
- formal release: 両backendの実装済みsupport matrixに対する4 OS L3。未対応targetを成功に見せるskipは禁止する。

現在の`gate.py`はselectorが要求したGo jobのsuccessを検証し、missing/skippedを失敗にする。実行case数とsummaryの整合性は`assert_go_backend_summary.py`が検証し、0 casesを失敗にする。target/toolchain receiptをaggregate gateで照合する仕組みは未実装であり、formal release coverageを広げる前に追加する。cache keyはbackend、compiler commit、toolchain、OS/arch、profile、features、dependency lock/sum、runtime hashを含む。外部PRのcacheはwrite不可または信頼境界を分け、artifactへsecret/private module credentialを入れない。

現在の`ci.yml`はpushでmain/tagを対象とし、任意の0.3.0開発branch pushを自動検査しない。初期sliceは必ずPRと`workflow_dispatch`で検証する。長期`0.3` branchを採用する場合だけ、権限を広げず明示branch triggerとrequired aggregate checkを追加する。CI時間短縮はrouting unit testsとshadow計測でmissがないことを確認した後に有効化する。

## Result / tryの入門改善

0.2.2では初心者向けの日英`Result`/`try`教材を公開済みである。`test-nagi-code/result-try-tutorial`で、`parse_i64`を使う完全実行例、手動`match`、`try`による成功値の取り出しとErrの早期return、Python例との違いを説明している。

Go backendでこの教材を実行できるとはまだ扱わない。残る作業は、`Result[Result[T,E1],E2]`、`await`との順序、error型不一致、HTTP responseへの変換を実装済みsupport matrixに合わせて追加し、Go対応後に同じsourceの両backend実行を検証することである。未公開Goコマンドを公開版手順に混ぜず、「例外を握り潰す」「panicへ変える」「自動rollbackする」という誤解を否定するnegative例も、実際の診断と実行結果を伴う場合だけ掲載する。

## 0.2.2との分離と次の判断

0.2.2へbackend選択、Go runtime、Go CI jobはbackportしていない。0.2.2はRust既定の高速化・安定化・Docs/IDE改善として、4 OS release検証とartifact再確認後に公開済みである。

0.3.0開発はG0/G1の最小実行系からG2の一部と、G3に必要なprivate lifecycle/cold Futureのfacts段階まで進んだ。各段階は個別commitと回帰testとして残す一方、reviewとmergeは統合branchのexact-head CIで行う。G3aではfinal ASTのcall/argument slotから再構築するCaptureStorageIdと、plain value graph/read-only view[str]だけのFutureInvocationExitを採用した。Goはsealed factsを消費し、await開始時のdetachと正常/Err/panic出口のactive invocation参照終了を実装する。physical memory即時回収・resource close・取消cleanupは保証しない。次の判断は、一般cleanupへ進むためのowner retirementとresource release anchor、Go extern binding構文、Go module local dependencyの許容範囲、owned-only adapter ABI、support matrixの初版である。現factsだけからreleaseを推測せず、HTTP/DB/auth/actor/task/scopeを登録だけで対応済みにしない。
