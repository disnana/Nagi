# Go backend 0.3.0: 最小実装の契約と実測

この文書は、Rustを既定のまま保つ単一の`nagic`へ、Goコード生成を段階的に追加する最初の実装記録である。Go backendは実験段階であり、Nagi 0.2.2へ含めない。ここにない機能を対応済みとは扱わない。

## 選択と互換性

選択順は次で固定する。

1. CLIの`--backend rust|go`
2. `nagi.toml`の`backend = "rust"|"go"`
3. 指定がなければ`rust`

```toml
entry = "main.nagi"
backend = "go"
```

```console
nagic check --project .
nagic run --project . --profile dev
nagic build main.nagi --no-project --backend go --profile dev
```

既存の`read_line()`を含むNagi構文やHigh/Low frontendは分岐しない。Highは従来どおり正規Low文字列へ変換し、その文字列を独立したLow parserへ通してfinalizeした後だけGoを生成する。手書きLowも同じfinalize境界を通る。型・所有権・capability検査をGo側の都合で省略しない。

`--backend`はproject設定より優先する。Goを指定しない既存projectは引き続きRustを使う。`--runtime-profile`、`--seed-lock`、`--incremental`、`--rust-diagnostics`はRust固有なので、Go選択時には無視せずエラーにする。

Rust backendで`[go].files`を宣言した場合と、Go backendでRust adapter/dependencyを宣言した場合もエラーにする。到達可能な`@rust` extern関数は、`main`からの関数依存chain、利用可能なRust binding、修正候補を表示してGo toolchain起動前に拒否する。Go native adapter ABIはまだないため、`[go].files`自体も明示的に拒否する。逆backendの資産を黙って捨てたり、backendを自動切替したりしない。

## 最小対応範囲

現在の実行可能な範囲は次のとおり。

- 整数、浮動小数、`bool`、`str`、`bytes`、`unit`
- 同期関数、import済み関数、local変数、`if`、`while`
- 値classの生成とfield参照
- `List[T]`のliteral・`len`・Copy要素のindex・`append`、`range`とCopy要素Listの`for`
- `Option`、`Result`、`try`、Option/Resultの`match`
- payloadなしenumのconstructor・型・網羅match。class、List、Option、Result内でも既存対応型の組合せに限り使用可能
- `std.ownership.move`による既存対応型の明示的な所有権移動
- 算術・比較・論理演算（Nagiの評価順序と`and`/`or`の短絡を維持）
- `print`、`write`、`assert_true`、`parse_i64`、`error_message`、`i64`とResult/Option constructor
- Highと保存済みLowの`check`、`lower`、`build`、`run`

Nagiの名前はUTF-8 byte列をGo identifierへ一意に符号化する。Go keyword、module修飾、Unicode、生成temporaryとの衝突で別のNagi名を同じGo名へ潰さない。関数引数や式はNagiの左から右の評価順を保持し、`try`の前処理が左operandより先に走らない。論理右辺の`try`も短絡時には実行しない。

整数literalはNagiの10進規則で正規化してからGoへ渡す。整数演算は既存profile契約を維持し、devの加減乗算と符号反転overflowをpanicにし、releaseではwrapする。符号付き最小値の`-1`による除算・剰余は両profileでpanicにする。浮動小数剰余、負のゼロ、有限値・無限大・NaNの`print`/`write`表示もRust backendの現在の結果に合わせる。GoとRustで最短10進表現の中点選択が異なる場合は、exact rationalで中点を確認してRust側の候補を選ぶ。

`std.ownership.move`は関数名の文字列ではなく、共有checkerが封印した`IdentityTransfer`だけをGoへ渡す。入力を左から右へ一度だけ評価し、型付きtemporaryへ値として移す。clone・deep copy・allocationを追加せず、非Copyのsourceは共有checkerの規則どおり以後使用できない。Copy値は移動後も使用できる。`move`という名前のユーザー関数は通常関数のままであり、`std.http.server.status`等の別の標準operationはこの対応によって受理されない。Go backendがまだ扱えない型を`move`で包んでも、Go起動前の型検査を迂回できない。

payloadなしenumは、共有checkerが封印したconstructorとmatch armのidentityをGo emitterへ渡す。Goではenumごとの固有整数型とvariant定数へ落とすが、数値変換や比較をNagiへ追加しない。matchの対象は一度だけ評価し、未知の内部tagはpanicする。payloadを一つでも持つenumは宣言単位で拒否する。enumを含む`main`のResult error型も、RustのDebug表示とGoの数値tag表示が一致しないため、この段階では拒否する。非`main`関数内のtyped Result/`try`/matchは既存対応範囲で使用できる。

次は未対応で、Go起動前にsource位置付きで失敗する。

- payload付きenum、enumのconsole表示、非Copy要素のList index/反復、Listのslice・field/index代入
- async/await、task、scope、actor
- HTTP、DB、auth/session、WebSocketなどRust runtimeの標準操作
- Go native adapter、Rust/Go間FFI、混成backend
- IDE管理runの`NAGI_RUN_RETENTION=latest`

この段階をHTTP等の本番対応やRust backendとの完全同等とは呼ばない。
対応する整数境界ではpanicするかwrapするかを揃えるが、panic本文、stack trace、process exit codeはbackend固有であり、現段階ではbyte単位の一致を保証しない。

## Buildと失敗時の保護

Go buildはapplication IDとgeneration namespaceをRustから分離し、選択した実行ファイルが実Go toolchainであることを`go version`で先に確認してから、staging directoryで`CGO_ENABLED=0 GOWORK=off GOTOOLCHAIN=local go build -trimpath -buildvcs=false -mod=readonly`を実行する。Goによる暗黙のtoolchain downloadは許可しない。成功後にimmutable generationを公開し、最後に`latest.json`を更新する。失敗したstagingは削除し、以前の`latest.json`と実行ファイルを維持する。

### backend間conformanceの次段

Gooseの複数実行方式テストは、同じ言語入力を複数targetで比較する方法の参考にする。ただしNagiへC/TinyCC backendや汎用backend plugin機構を追加する計画ではない。まずRust/Goの共通対応範囲に対して、次を同じoracleへ段階的に集約する。

- 成功例はstdoutと終了statusを正規化せず完全一致させる。
- compile failureは失敗stageと必須diagnostic部分、元source位置を照合する。
- runtime failureはnon-zero statusと期待した構造化stderrを照合する。両targetが無関係な理由で失敗しただけでは一致としない。
- High、保存Low、手書きLowのうち各機能が契約上必要とする入口を区別し、未対応targetは明示的なunsupported診断を期待する。

時間、memory、schedule順序の一致は意味論一致へ含めない。actor/task/auth等をGoで対応するときは、それぞれのlifecycle・取消・resource契約を別途満たすまで共通対応に数えない。

最初の共有runnerは、既存のRust専用42 corpus、bounded生成、batch、fuzz、shrinkerを変更せず、明示した少数の同期programだけをRustとGoで実行する。成功はstdoutとstderrのraw byte、終了statusを期待値へ個別に照合した後でbackend間も比較する。CRLF、Unicode、空白を正規化しない。High、Highから独立保存したLow、手書きLowを別入口として数え、欠落した入口を動的filterで成功扱いにしない。

runtime faultはnative build成功後のnon-zeroだけを対象にし、事前に出したmarkerのraw stdoutとbackend別のprimary fault構造を要求する。RustとGoで異なるpanic exit codeやstack traceは同一bytesを要求しない。compile拒否、toolchain欠落、spawn失敗、timeoutはruntime faultへまとめない。期限切れではUnixの専用process groupまたはWindowsのprocess-tree終了を使い、直接childだけを止めてcompiler子孫を残さない。`nagic run`が追加するCLI stderrではなく、成功generationの実行ファイルを直接観測する。現段階の共有例は整数・Unicode文字列・List・明示move・payloadなしenum、境界外index、共通checker拒否、Goのview未対応拒否であり、全corpusのbackend同等性を達成したとは扱わない。

`out/go/main.go`と`go.mod`は互換表示用で、既存fileへ書き込まず、専用temporaryを作ってdirectory entryを置き換える。既存hardlinkを通じて旧generationを書き換えず、symlinkやplain directoryでない投影先は失敗させる。投影失敗時は新generationが孤立する場合があるが、旧`latest.json`と旧実行ファイルは変えない。

generationには次を保存する。

- `generated.low`、`main.go`、`go.mod`、source snapshot
- compiler、入力、生成物、実行ファイルのSHA-256とbyte数
- Go version、GOOS/GOARCH/GOVERSION/GOROOT、module graph、profile、CGO無効化

これはbuild receiptであり、まだGo成果物の独自cache hit判定には使わない。source snapshotはcheckerが読んだtextだが、外部入力fileのhashはbuild後の観測なので、build中に変更されたfileまで同一snapshotだったと証明する完全なinput keyではない。Go自身のbuild cacheを使い、入力条件を証明できないNagi独自cacheでnative成果物を再利用しない。

## 実行検証

専用integration testは実Go toolchainを使い、private lifecycle初段ではLinux/macOSで27件、WindowsでUnix symlink固有の1件を除く26件を検査する。

- Highと独立保存Lowが同じ出力になる
- private std-only lifecycle kernelの12 native testを名前集合まで検査し、public std.actorのHigh/手書きLow事前拒否を維持する
- CLI &gt; project &gt; Rust既定
- import済み関数
- List literal/index/append/len、Copy要素Listとrangeの反復、左から右へ一度だけの評価
- 非Copy要素Listの反復をGo toolchain起動前に拒否する
- High・保存Low・手書きLowの明示move、alias/module-qualified呼出し、Copy/非Copy、`try`との合成
- High・保存Low・手書きLowのpayloadなしenum、import alias、宣言順と異なる網羅match、Copy/move後の再利用、List反復
- High・保存Low・手書きLowの小さな共通programをRust/Goで直接実行し、raw出力とstatusを一致させる
- 両backendの境界外indexをruntime faultとして区別し、Goのview拒否とtoolchain欠落を別段階として扱う
- payload enum、enumを含むmain error型、enumの未対応表示をGo toolchain起動前に拒否する
- use-after-move・二重moveと、未対応の別標準operationをGo toolchain起動前に拒否する
- Result/try、class、評価順序、短絡、identifier衝突
- Option/Resultの完全matchが全armからreturnする場合
- 先頭ゼロを含む10進整数literal、dev/releaseの整数overflow、符号付き最小値の除算・剰余
- 浮動小数剰余、負のゼロ、最短表示の中点、console表示、文字列内容によらないimport
- 未対応機能がGo起動前に失敗する
- Rust externの到達chain
- Rust/Go資産の逆指定を拒否する
- Go adapter未対応を明示する
- Rust固有optionを黙って無視しない
- Go toolchain不在ではgenerationを公開しない
- compatibility projectionのhardlink/symlinkから旧generationを変更しない

frontend/backend拒否後のlast-good維持は検査した。成功generation後に外部Go process自体を意図的に失敗させるportableな注入test、output lockの並行競合、power-loss耐性はこの段階では未検査であり、last-good保証をその範囲まで一般化しない。

`cargo test --locked`がこのtest binaryを含むため、compiler/runtime変更に対する既存のcomprehensive Linux gateでも実行される。専用3 OS jobと差分routingはDraft PR #179で追加し、Go jobのmissing/skip/0件実行を`Ready to merge`で拒否する。shared frontend変更は両backend、target固有変更はそのbackendとshared invariant、未知pathとselector/gate変更は広いcoverageへfail safeする。#179は未マージであり、この文書の実装branchはそのHEADをbaseにしている。

## 2026-10-26の小規模build実測

同じPoint、関数、`Result/try`、console出力を含む小規模programを、release buildした`nagic`で測定した。hostはLinux x86_64、AMD EPYC 9V74の割当5 CPU、Go 1.27.3、Rust 1.99.1。`nagic`自体のcompileとdependency downloadは時間に含めず、`nagic build`開始からapplication executable完成までを測った。Go cleanは空のoutputとprivateで空の`GOCACHE`から始め、以後同じcacheを再利用した。Rust cleanは空のoutput/targetから始めたが、registry sourceは既にlocalにありnetwork取得はない。Rustは`--profile dev --runtime-profile auto --incremental`で、今回のprogramではfull runtimeが選ばれた。wall timeはprocess外から、backend内訳は`--build-stats`から取得した。

| ケース | Go wall | Go tool | Rust wall | Cargo |
|---|---:|---:|---:|---:|
| clean 1回 | 4.330 s | 4.309 s | 45.283 s | 45.168 s |
| 変更なし | 0.180 s中央値（4回、0.167〜0.315 s） | 0.157 s中央値（0.145〜0.293 s） | 0.570 s中央値（3回、0.545〜0.571 s） | 0.449 s中央値（0.428〜0.454 s） |
| 関数本体1箇所変更 | 0.186 s | 0.166 s | 0.638 s | 0.532 s |

Go integration test 14件は最近の実行で42.37〜86.63秒だった。toolchainとbuild cacheの状態を固定したbenchmarkではなく、overflow profile、projection保護、浮動小数境界を含むため、この時間や修正前の10件版5.541秒をbackend速度の比較に使わない。

この比較は最小subsetのbuild feedbackだけを示す。Go側にはRust runtimeのHTTP/DB/auth/task等がまだなく、依存量が同じではない。native実行性能、throughput、latency、長時間安定性、Windows/macOS、release profile、最大RSSはこの測定で比較していない。process-tree RSSの簡易observer値は妥当性を確立できなかったため採用しなかった。機能同等のbackendができるまで、本番性能の優位性をこの値から主張しない。

## 次段階の受入条件

#186後の最初の並行実行sliceは[private lifecycle kernel](go-lifecycle-kernel.md)。
std-only Go moduleでgeneration・one-for-one再起動・global予算・実joinと明示cleanupを
native検査する。生成applicationへの組込みとpublic std.actor受理はまだ行わない。
mailbox/READY/events/charge、cold async/borrow/callback/owner cleanupは後続gateである。
このsliceを含むGo integrationの必須件数はLinux/macOS 27、Windows 26。

1. 3 OSで最小backendの実build/runとlast-good保護を必須CI化する。
2. shared conformance oracleとbackend別driverを分離し、同じ対応Nagi programの結果・診断を比較する。
3. Go native adapter ABIを設計してから、標準runtimeをconsole以外へ広げる。
4. task/auth/admission/cancellationはgoroutine/contextやGC cleanupへの単純置換をせず、Rust側のlease・join・error propagation契約を個別に証明する。
5. HTTP、DB、auth、task/actorの順に機能同等性、race、cancel、resource limit、soakを通す。
6. 機能同等の実用appでcold/warm/edit/test compile/test execution、disk、RSS、runtime latency/throughputを複数回測る。

各段階で未対応機能は明示エラーのまま保ち、安全性や意味論が証明できない境界ではRust backendへ自動fallbackしない。
