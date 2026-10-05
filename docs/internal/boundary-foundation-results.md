# #74を土台にしたバックエンド境界の検証

2026-10-05、Linux x86_64、Rust 1.98.1。作業開始時の基点は未マージの[#74](https://github.com/disnana/Nagi/pull/74)、head `6d5b9eec4f0101fd501cb527e8772b1fbcfcd514`。その変更、Low互換性、Rust backendを維持し、別ブランチで作業した。その後ユーザーが#74をmainへマージした（`04d30b5`、treeは#74 headと一致）。追加分の[#76](https://github.com/disnana/Nagi/pull/76)もユーザー側でmainへマージ済み（`8f6cc6c`、treeは#76 head `13b59aa`と一致）。以下は#76作業時の検証記録であり、次フェーズの検証結果ではない。

## 判断と参考資料

監査は[boundary-foundation-audit](boundary-foundation-audit.md)、外部資料と採否は[backend-boundary-research](backend-boundary-research.md)に記す。前フェーズのCodex/AGENTS・rustc/LLVM/Clang/Swift・生成/縮小の調査は[compiler-testing-research](compiler-testing-research.md)を引き継ぐ。

Rust Referenceからprofile依存overflowと常時失敗の区別、Cargoから構造化診断、Axumから同じRouterでの処理比較、Serdeから所有DTOとborrowed decodeの区別を採用した。OWASPからresource単位の判定とDB操作時の再確認を取り入れた。rustcのMIR定数伝播、任意crateの自動解析、独自暗号・HTTP輸送・DB driverは実装しない。

採用判断は[ADR 001](adr/001-backend-boundaries.md)、[002](adr/002-constant-validation.md)、[003](adr/003-diagnostic-boundary.md)、[004](adr/004-checked-boundaries.md)。専用HIR/CheckedProgramへの全面移行は保留し、#74のchecked operand・origin・cleanup planを残した。型・所有権checkerはまだ大きいwalkerであり、表に段階を書いたことを実装の分離完了とはしない。

CI後に追加した[ADR 005](adr/005-native-artifact-identity.md)は、共通Cargoキャッシュとアプリの実行ファイル名の契約を定める。環境変数の有無ではなく、既定キャッシュにも同じ識別規則を適用する。

## 共通原因と修正

| 分類 | 問題・原因 | 修正と観測 |
|---|---|---|
| P1 | literalだけのゼロ検査では`1 / (1 - 1)`やscalar aliasを後段へ通す | 型検査後の共通constant validation。8整数幅、binding ID、継続枝join、loop write killを使う。MIN/-1・MIN%-1も検査 |
| P1 | signed MINの二重否定が、Rustの範囲外正literalとして印字される | typed MIN leafだけを`primitive::MIN`として印字。外側の式はfoldせずdebug panic/release wrapを保持 |
| P1・新実験内 | opaque errorをmainから返すと、生成側が存在しないDebug条件を要求する | native registryのDebug契約を使う。nonDebug Errは固定messageと終了code 1、proofの中身を表示しない |
| P1・新実験内 | 明示`share`だけの検査ではApp/Supervisorの内部Arcへproofを格納できる | native registryに共有payloadの位置を置き、署名・constructor両方を共通の型検査へ通す |
| P2 | function signatureやnative phantom型を実payloadと混同する。深さ上限を超えるDTOもproofと誤判定する | nominal field graphを反復走査し、registryの実payloadだけを辿る。関数署名とphantomを共有値・field payloadとして数えない |
| P2 | 元Nagiの診断の後に生成Rust本文を重ね、原因を追いにくい | mapped primaryと関連causeを通常表示。`--rust-diagnostics`で詳細を表示し、unmapped/native/依存errorはrawを保つ |
| P1・CIで発見 | 既定native cacheでは同じstemの別生成projectが同じexeを上書きし、Cargo終了後のrunが別アプリを実行し得る | 既存のcanonical(source,out) identityを既定・明示cacheで共通にする。Cargo終了後に別buildを挟むbarrier回帰で、修正前の取り違えと修正後の分離を確認 |

新しいP0は今回の対象と検査では確認していない。リポジトリ全体の安全性証明や独立したsecurity scanを行ったという意味ではない。AuthのP1は今回作った実験のレビューで発見した契約違反であり、既存公開版に同じAPIがあったという報告ではない。

## checkが確認するもの

- 対応するNagiの名前・型・move・view origin・Result・control flow。HighとLowに同じ規則を適用する。
- 型付き定数モデルで分かる整数ゼロ除算とsigned MIN除算overflow。型・名前のエラーを新しい定数診断より優先する。
- schemaを明示した既存SQL checkの列・必要返却列・bind数。通常checkにSQL検査を黙って追加しない。
- 保護APIの署名が要求するPrincipal/Grantと権限型、move後の再利用、構築・JSON復元・copy/shared禁止。内部Arc stateも共有境界として扱う。

サポート範囲のNagiを受理した後、Nagi側で分かる型・move・lifetime問題で生成Rustが拒否されるのはP1として扱う。[言語契約](language-invariants.md)に保証目標とRustへの委譲を記す。探索で反例が出なかったことは、この目標を全プログラムについて証明したことではない。

## checkが確認しないもの

任意関数の定数評価、全rustc lintと同じ受理集合、任意Rust trait/unsafe契約、crate実装、link/target環境は確認しない。未知の整数値を安全な除数と証明せず、実行時のゼロ除算はpanicとなる。`+/-/*`overflowは従来のdebug panic/release wrapを維持する。

Authは宣言した保護APIの受け渡しに限る。全route、SQLのtenant条件、通常DTOのresponse流出、期限・失効・競合、背景処理へのdelegation、policy内容は静的に証明しない。`NAGI-AUTH-001`によるroute全体の検査、一般effect/taint解析、新route構文は未実装。

## Rust連携と認証・認可

[Authサンプル](../../test-nagi-code/application-examples/auth-boundary/README.md)は既存typed externとCargo依存を利用する。AxumがHTTP、Rustがcredential検証、名前付きNagi async関数が独自policy、RustがGrantの発行と保護SQLite操作を担当する。保護操作はGrantのsubject/resourceを使い、別のbare IDは受け取らない。SQLでもowner/blockedを再確認する。

通常classは入力・claims・業務errorの型として有効だが、構築できる値を認証成功のproofにはしない。Rustのprivate fieldを持つPrincipal/Grantには、trusted Rust issuer用の公開生成APIがある。Nagiからは直接作れない。虚偽のextern/issuer、迂回するRust/DB操作、常に許可するpolicyをcompilerが見抜く仕組みではない。

JWS署名・algorithm・issuer・audience・期限・鍵管理は既存Rust verifierへ接続する設計で、今回の固定credentialデモには実装していない。暗号をNagiで再実装しない。SQLx/SeaORM/Scylla/Redis/Valkey用adapterや利用者定義の任意opaque資源も未実装。async関数を名前でawaitできることと、任意のasync関数値を境界へ渡せることは別である。

認可フローは日英READMEの手書きMermaidで確認できる。自動生成は[ADR 004](adr/004-checked-boundaries.md)に後続候補として記す。現在のmapがRust内のpolicy経路や全情報流を推定・証明するとは説明しない。

## 診断の精度

High/Lowの生成行から元のNagiファイル・文の行へ戻す。importしたmodule、adapter型不一致、async Send失敗、primaryと関連causeは既存・追加testで確認した。式の正確な列、全Rust span、Rust editのNagiへの変換は未対応。手書きRust・依存・合成位置は推測でNagiへ移さない。既存library APIの詳細表示は維持する。

## 実行した検査

| 検査 | このフェーズの結果・範囲 |
|---|---|
| 初回全suite | `cargo test --locked`成功、登録762 tests（doctest含む）、ignored 0。明示的なnative targetを指定しており、既定経路の競合は見逃した。子processのstdoutを再集計して件数を増やさない |
| 静的品質 | fmt、全target clippy `-D warnings`、変更workflowのactionlint成功 |
| 定数 | 新規10 test関数。8幅、profile、type優先、binding/branch/loop、Unicode import位置、check/build前拒否とHigh/Low/native |
| Auth | 新規9 compiler test関数、2 runtime test、5 Rust compile-fail doctest。missing/fake/wrong-permission/reuse/JSON/共有・phantom・nonDebug mainを検査 |
| corpus | 31→38 source。新規7小再現。実runtimeが必要なものは11 linked harnessへ接続し、全suiteで実行 |
| 生成 | 10→13 bounded grammar。function value、複数borrow source、Result/Option、最初のpollで終わる純asyncを追加。2 seed×256生成＋38 corpusをnative実行 |
| mutation | 2 seed×10,000 input、panic 0。parse拒否14,236、check拒否3,814、check後Low/emit成功1,950。nativeは別の限定生成128×2だけ |
| 実HTTP | High・保存Low各52 case、計104成功。両policy modeで200/401/403/404、入力・本文上限、panic/timeout後の応答、停止とlistener解放 |
| CI補助 | Python CI scripts 51 tests成功、corpus/harness登録確認成功 |
| VS Code | 最新compilerをPATHへ指定してNode 196 tests成功。最初の未設定実行はcompilerを利用する11 fileが失敗し、環境を直して再実行した。実IDE UI・JetBrainsはこのフェーズで未実行 |
| 文書 | 日英公開ページ90ページをbuildし、ローカルlink・anchor・asset確認成功。変更Markdownの相対リンクも確認 |
| CI後の回帰 | 明示共有と既定共有を同一cwd・別source/outで実行。修正前はfirstがsecondを実行して1 pass/1 fail、修正後は2 pass。環境変数を除いて既定経路を強制する |
| 修正後の関連検査 | `NAGI_NATIVE_TARGET_DIR`を除き、Auth 9・project 12・shared target 2、計23 tests成功。VS CodeとHTML viewportは計201 tests成功。最初の制限付きNode実行は同期child spawnがEPERMになり、実行環境の権限を揃えて再確認した |
| 修正後の全suite | `NAGI_NATIVE_TARGET_DIR`を除いて`cargo test --locked`成功、登録763 tests、ignored 0。fmt、全target clippy `-D warnings`、Docs 90ページと変更Markdownの相対リンク確認も成功 |

詳細commandと保存/縮小の上限は[compiler-testing](compiler-testing.md)。Proptestを導入する代わりに、既存のbounded generator・独立oracle・budget付き縮小を拡張した。coverage-guided fuzz、全CFG/Scope/任意asyncのランダム生成ではない。Scope・取消・panicは専用実runtime regressionで検査する。

## 性能とFuture

[比較条件・raw結果](../../benchmarks/results/auth-boundary-2026-10-05/README.md)を保存した。手書きRust modeのhandlerはNagi関数を呼ばず、Nagi modeは生成load/policyを呼ぶ。同じAxum Router、DB、DTO、制限、実行ファイル、2 runtime worker、8 keep-alive接続で各3 run。全runのtransport/status/body errorは0。

qps中央値はNagi 62.7k／Rust 64.1k、p99中央値325／354µs、RSS snapshot約4.2–4.4MB。分布が重なる共有hostの短期測定で、性能優位、最大負荷、長期稼働SLOを主張しない。policy/mint/consume microの呼出thread allocationは0だが、HTTP・DB・schedulerを含まない。Auth例のFutureはpolicy 72／64 byte、load 168／144 byteで、差は残っている。

共有binaryは4,008,032 byte。同じ生成projectをdependency cacheから3回rebuildし約5秒。独立Rust binaryとのsize/compile時間差、cold build、Go、TLS、外部負荷機は未測定。

### #74で観測した32 byte差

同じRust 1.98.1・x86_64でdebug/最適化の両方を測った。

| borrow historyとawaitの条件 | Future byte |
|---|---:|
| Nagi生成、local view→caller view、generated extern wrapper | 168 |
| 元の手書き比較、caller viewのみ、native pause | 136 |
| 手書きで同じlocal→caller history・2 slots、native pause | 160 |
| 手書きで同じhistory・2 slots・同じgenerated wrapper | 168 |

元の136 byte比較はborrow historyまで同じではなかった。MIRには生成版の2つのOption<Vec> slotと、caller-only版の1つのVecが残る。VecとOption<Vec>はこのtargetで24 byte。native pauseのFutureは1 byte、extern wrapperは2 byteで、wrapperだけの差でも全frame配置は8 byte変わった。同じhistory/bridgeを持つ手書き実装は生成版と同じ168 byteだった。

観測した32 byteはslot/lifetime historyの24 byteとawait wrapper/layoutの8 byte差に分けられた。ただし全Futureや全targetのABI・最適化を証明しない。payload cloneで小さくしたり、Drop順を変更したりはしていない。

再実行は`NAGI_TEST_ARTIFACT_DIR=/tmp/nagi-future-layout cargo test --locked -p nagic --test view_container_drop`。保存される`high.stdout`、`saved-low.stdout`、`handwritten-low.stdout`に4条件のsizeが出る。各sourceの資源破棄・Err/unwind・pending取消のassertは維持する。

## CIとレビュー単位

PR/pushのRust変更では既存Linux全suiteとfuzz smokeを使い、4 OS対象のnative contract suiteにもconstant/Authを追加した。Docsだけの変更はRust全suiteを起動しない。週次/手動workflowは2 seed、各256生成、10,000 mutation、128 nativeと重要回帰を実行する。失敗は縮小結果とstageをartifactへ残す。

このフェーズのローカル結果はLinuxのみ。#74の4 OS成功を新差分の成功として転用しない。#76の最初のCIではLinuxとmacOSのAuth実行結果が空になり、既定キャッシュの上書きを発見した。ローカルの明示共有設定で成功した結果を、既定経路の保証へ広げてはいけない。

実行ファイル修正後のCIでは、Windowsの生成Rust診断の`\\`区切りと、macOSの一時ディレクトリの`/var`・`/private/var`をテストが別物と扱った。診断の行・detail有無は維持して区切りを揃え、実行ファイルとproject targetはcanonical pathで比較する。アプリのstdout、別binary、同target、source行のassertは削らない。これらはP2のテスト移植性の問題で、生成Rustやruntimeを変更しない。

最終head `13b59aa`の[CI run 37291388126](https://github.com/disnana/Nagi/actions/runs/37291388126)は成功。Linux全suite、Windows x64・Linux x64・macOS Apple Silicon・macOS Intel、VSIX、IntelliJ IDEA・PyCharmの検査とmerge gateが完了した。[website run 37291387847](https://github.com/disnana/Nagi/actions/runs/37291387847)も成功。publish-releaseはskipであり、この結果をrelease公開確認として扱わない。実IDEでの手動UI操作や、全プログラムの正しさの証明ではない。

この表記対応後、Linuxでbuild diagnostics 30・project 12・shared target 2の44 tests、fmt、全target clippyが成功した。Windows/macOSの成否は再CIで確認する。独立レビューでもnegative・元位置・barrier・binary区別の検査を維持していることを確認した。

#74へ積まず、別Draft PR #76として分離した。作成時は#74のbranchがbaseだったが、ユーザーの#74マージを確認してmainへ変更した。定数・診断・Auth・CI由来の成果物修正をcommitで分ける。Authは標準APIの安定化前の実験と明記する。Copilot reviewは実行できず、成功レビューとして数えない。

## 残す問題と次の3項目

| 分類 | 残課題・理由 | 次の行動 |
|---|---|---|
| P1候補・継続探索 | 対応範囲のcheck/build不一致は全ケースで排除できたと証明していない。constant propagationもrustcと同じ解析ではない | 反例を保存・縮小し、同じorigin/use/constantモデルの欠陥か検査してから修正 |
| P2 | ASTのoptional型・大きいchecker・Low text再解析・statement-line mappingが残る | CheckedProgram/source identityの段階移行を具体例と費用で評価 |
| P2 | Auth proofは固定i64 subject/resourceとnominal permission。任意opaque resource、全route/effect、request寿命には未対応 | trusted issuer・policy・資源契約を先に設計。曖昧な推測や独自暗号を追加しない |
| P3 | await bridge/私有slotによるFuture差とcompile-time費用 | 同じborrow history・取消・Drop条件で計測し、証拠がある箇所だけ縮める |

1. 新差分を4 OS CIと独立レビューへ通す。特にAuthは型の受け渡しとpolicyの正しさを別に確認する。
2. 次のcheck/build反例をbounded corpusへ追加し、関数値・複数borrow・async/Scopeの組合せを拡張する。全面IR rewriteは始めない。
3. 実アプリで必要になったRust資源adapterとsource mappingを小さく育てる。JWS等は既存crateの検証契約から設計し、フロー自動表示は主作業の後に扱う。

現時点で保証できるのは、明示した言語契約と、上記の対象ケースを継続検査する仕組みである。任意のNagi/Rustプログラム、全認可経路、panic後のrollback、未知のbackend拒否、未実行targetの正しさまで保証しない。
