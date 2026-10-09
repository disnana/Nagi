# JetBrains semantic assistance: 内部compiler契約

未リリースの追加機能。既存配布0.1.11にこのcommandがあるとは扱わない。
`symbols` の `nagi-symbols-v1`、`check`、既存 `--editor-input` schemaを維持する。
IDEに型・scope・import・move・view規則を再実装しない。

## 最小APIと解析の責任

`nagic assist --project /project/nagi.toml --editor-input` は設定entryとnativeを解析する。
standaloneは `nagic assist /project/file.nagi --no-project --editor-input`。
既存CLIと同じく、明示SOURCEのみの指定はmanifest自動探索を行わない。
SOURCEと `--project` の併用はentryだけを変更しnative設定を保持するため、
root側にしかないreplace対象がactive-file解析で消える可能性がある。
projectのentryを解析し、loaded import/native graph内のactive fileをqueryする。
query fileがgraphに含まれなければ補完を返さない。

stdinは1個のJSON。queryは省略可能。

```json
{
  "files": [{"file": "/project/main.nagi", "text": "def main():\n    fresh = True\n    fr\n"}],
  "query": {"file": "/project/main.nagi", "line": 3, "column": 7}
}
```

files/queryは既存fileをcanonicalizeする。相対pathはcompiler cwd基準。
filesは.nagi/.low、重複不可、128 files・合計8 MB、stdinは16 MBまで。
source loaderのimport上限（depth64、files128、合計8 MB）も維持する。
line/columnは1始まり、columnはUTF-16。queryをUnicode scalar/token spanへ明示変換し、
surrogate中間や範囲外はsemantic factsを返さない。tab禁止は言語lexerの診断を維持する。
保存前のbufferはoverlayのみで解析し、diskへ書かない。存在しないphysical fileは未対応。

stdoutは1個の `nagi-assist-v1` JSON。

```json
{
  "format": "nagi-assist-v1",
  "semantic_status": "editor-partial",
  "frontend_checked": true,
  "frontend_accepted": false,
  "full_compile_checked": false,
  "recovered": false,
  "completion": {
    "kind": "names",
    "access": "read",
    "items": [{
      "name": "fresh", "kind": "local", "type": "bool",
      "target": {"file": "/project/main.nagi", "line": 2, "column": 5, "length": 5},
      "borrowed": false, "access": "read"
    }]
  },
  "diagnostics": [{
    "severity": "error", "stage": "check", "message": "original compiler diagnostic",
    "file": "/project/main.nagi", "line": 3
  }],
  "symbols": {"format": "nagi-symbols-v1", "references": []}
}
```

例のsymbolsは省略表示。実際は既存definitions/bindings/references/locals/expressions/files/
standard_sources/standard_modulesを含む。`symbols.references` のlocation→targetをnavigationに使う。
lexical binding resolutionは型error/moved valueでも有効。`symbols.locals` やexpressions.fieldsを
所有権上の補完候補として使わない。補完候補は `completion.items` のみ。

kindはnames/members、item.kindはlocal/field/function/class/resource/enum/module等。
local/fieldはtype、globalはsignatureを持つ。targetは任意（fieldの宣言span不明時はnull）。
namespace itemのaccessはnamespace。それ以外はread。borrowedは借用/共有receiverを含む。
これらはcheckerがその位置で検証した参照候補であり、move/代入/任意call引数としての
挿入を承認するものではない。消費・可変操作は編集後の通常compiler診断で検証する。
無条件のbuiltin名リストや未検証のsemantic candidateは追加しない。

## 同じcheckerと保守的な回復

queryはloaded-source lineとoriginal-file token spanを指定する。
既存Checkerがその式を評価する直前のvars/ownership環境を観測し、通常expression checkerで
name/fieldのread probeを検証する。部分field move後は利用可能な兄弟fieldだけを返す。
scope/shadowing、条件move、loop固定点は既存checkerの状態を使う。loopが同じ式を再訪した場合は
全観測で有効な候補の共通部分を返す。unavailable localも同名globalを隠す。
module namespaceの候補はresolverのbinding/own-exportとこのcheckerの署名検証から得る。
標準resourceのfieldは既存stdlib registryとcheckerから得てIDE側の一覧に複製しない。
native Lowおよびreplacement bodyも同じcheckerでqueryする。

以前のstatementが失敗した場合、editor modeのvars rollbackは補完を許可する根拠にしない。
その関数の後続queryは空候補を返す。navigation/declaration/type hintsは別契約で残せる。
この保守性により、独立したtype error後も補完が空になることは既知の制約。

回復はcursorで終わる単一行だけ。simple dotted placeの最後のdot/member prefixをspaceで消し、
receiverのtoken spanをcheckerで検証する。blank expression statementはTrueに置換する。
complete parserが扱えるidentifier prefixは元ASTを使う。call/index receiver、未閉鎖call、
複数箇所のsyntax error、Lowのsemicolon前の未完memberなどは回復しない。
回復した行のsynthetic tokenはnavigationに公開しない。他の行、receiver、元file位置を維持する。
回復前original sourceの診断とfrontend判定だけを表示する。syntax拒否をcompiler受理へ変えない。

## 診断と成功の境界

original inputを通常High check→生成Low再parse/check→native統合/finalizeで検査する。
Low inputも通常finalizeへ接続する。output directory、生成Low/Rust、cache、rustc/Cargo実行は不要。
SQL opt-in検査はこのcommandの契約外。
`frontend_accepted` はこの境界だけの結果で、rustc/native acceptanceを意味しない。
`semantic_status:editor-partial` は回復を含むeditor factsで、完全なcompile successではない。
`full_compile_checked` は常にfalse。

診断は最初のcompiler errorを返す。既知のoriginal source file/lineのみ構造化する。
checkerのline-only診断にrangeを発明しない。parser/lexerがscalar columnを報告する場合だけ
元sourceからUTF-16へ変換し `range:{line,column,length:0}` の正確なpointを返す。
pointはtoken幅を意味しない。originのないloader/cycle等のmessageはfile/lineなしも許容する。
messageは既存日本語/compiler表示を維持する。

正常parse/check拒否はexit0+JSON diagnostic。invalid schema/path/limit/configは非zero+stderr。
IDEは後者やJSON/version不一致でresultを破棄し、saved sourceや独自checkerへfallbackしない。

## IDE側の世代・trust・performance

compiler protocolはepochを保存しない。one-shotと `assist --serve`（改行区切りJSON）を提供し、IDEはproject/command plan単位の常駐processを再利用する。requestは直列化し、取消・timeout・protocol失敗はprocessを停止して次requestのresponse混同を防ぐ。各requestのframeは16 MB、responseは8 MBに制限し、requestごとにmanifest/overlay/sourceを読み直す。IDEがlaunch時にproject、全buffer version/content、
query、compiler executable/config/native/import identityをcaptureする。どれか変化すれば結果を捨てる。
diagnostics/completion/navigationとも同じsnapshotに属する。project switch/dispose/cancelはprocessを
kill/reapし、古いsnapshotを新bufferへ適用しない。project trust承認前はcompilerをlaunchしない。
これらはJava project serviceの責任でありcompiler responseの受信だけでは保証しない。

debounce/cancellation/cacheはIDE側。同じsnapshotの解析失敗も記録し、annotatorの再実行で同一失敗processを起動し続けない。変更後は新snapshotとして再解析する。利用不可・未信頼・未保存manifest・上限・失敗の理由はcodeのcompiler errorと分け、file-level editor noticeで示す。compilerはsourceの再parse/checkを行いincrementalとは呼ばない。
query取得済みtyped programをsymbols serializationへ再利用し、binding位置はfileごとに1回lexして
引く。candidateごとのfile再lexとloop intersectionの二重全走査を避ける。
実測条件/latency/候補数/失敗入力をartifactsに保存し、最終IDE latencyはJava fixtureで別検証する。

## 検証記録

REDは空candidateの初期assist stubに対しunsaved `live:bool` candidateが欠落するcontract assertion。
raw logs: `/workspace/nagi-jetbrains-semantics-artifacts/`。
compiler/tests/editor_assistance.rsはunsaved local型、move/scope/shadow、partial field move、shared read、
imported overlay、module own exports、unsafe statement、unsupported syntax、UTF-16 navigation/point、
surrogate中間、original module line-only error、cycle/tab/input limit、branch/loop move、native replacementを扱う。
既存symbols/module_symbols/project互換とnative High/Low harnessの結果は実行後追記する。
全OS、IDE世代競合/取消、trust、実UI/navigation/completion表示はこのcompiler fixtureの保証範囲外。

現在のnavigation制約: reused symbolsはuser-record fieldの宣言tokenを返さない。
record member completionのtargetはnullで、完全なrecord-field Go To Definitionには追加の正確な
parser field spanまたは曖昧でない検証済みdeclaration token lookupが必要。resource fieldは既存registry位置を使う。

初期試作のtargeted検証: assist12、symbols21、module_symbols5、project12、conformance6、
explicit_moves9、frontend_contracts13、shared_field_moves4、view_flow_completion3、view_origins10はpass。
`clippy --locked -p nagic --all-targets -- -D warnings`、fmt check、diff checkもpass。
通常frontend native実行は初回registry/proxy接続でinfra failure。
CARGO_NET_OFFLINE=trueと既存warm NAGI_NATIVE_TARGET_DIRでnativeを再検証してpassした。
warm targetを使ったnative成功をclean build成功とは扱わない。

debug one-shotの10/100/500 local benchmarkは、候補数/診断をassertして測定した。
同時native build中のp50は約1.10/1.35/1.61秒、最大4.07秒。
別のwarm build中のCPU付き測定はp50約1.03/1.37/1.39秒、最大5.48秒。
この初期試作時点ではoptimized benchmarkを引継ぎ、IDE end-to-end latencyは未確認だった。
raw JSONと再現scriptはartifactsのperformance-debug*.json / measure_assistance.py。

## 2026-10-09 継続検証（公開前候補）

main `3b8da226eb26c27187f3b0bc4fa39639abe4ac57`へ保全した試作をrebaseした。
0.1.3は候補versionであり、公開0.1.2の説明へ混ぜない。
compiler契約は17件成功（High・保存Low・手書きLowの通常frontend判定、常駐processのoverlay更新、invalid frame拒否を含む）。
workspace全体は101 result blocks・1012 passed・0 failed・1 ignored。ignoredは既存Task cost測定。
このworkspace実行時点のassistは15件で、その後追加した2件はtargeted 17件として実行した。
fmt、workspace all-targets clippy、bounded fuzz-smoke（text1000/native16、panic0）、release scripts116件、CI scripts63件、website102ページも成功。

optimized release compilerの常駐protocol測定は、7回ずつ10/100/500 local、p50約166/182/318 ms、最大186/188/363 ms。
compiler処理とtransportを含み、IDE debounce・PSI・描画・cache hitは含まない。同時Gradle検証中の共有Linux環境、warm build/cacheでの値。
同条件の独立one-shot測定p50約211/447/647 msは別の測定時刻・競合状態のため、比率を高速化の保証にしない。
再現scriptは `benchmarks/editor-assistance/measure.py`、原JSONは `/workspace/nagi-jetbrains-semantics-artifacts/performance-*-release*.json`。
process再利用はincremental checkerではなく、毎requestのsource graphを再checkする。

標準Run Configurationは既存run command planを再利用し、project entry/native設定、trust gate、明示run前保存、Stop時process tree終了を維持する。
左上RunはNagi configurationを選択後、gutter/contextからのRunはそのconfigurationを生成する。
Windows実GUI、入力から表示までのlatency、利用者GUI試験は未確認。
4 OSおよび4 IDE SDK/Plugin Verifierは今回候補HEADのCIで別に確認し、過去0.1.2結果を代用しない。
GUIチェックリストは [試用手順](jetbrains-semantic-assistance-gui-checklist.md)。

## 独立reviewの修正記録

`e2538b9`をSol Highが独立read-only reviewし、2件のP2を指摘した。
closed import/nativeの返答後VFS stamp追加だけでは、disk→VFS通知遅延中の変更を見逃す。
未知graphを含む最初のresponseを捨て、known graphのbounded disk contentをworkerでpre/post照合してからpublishする。
上限は128 dependency files＋manifest、合計8 MB。snapshot内容保持とpost比較用bufferにmemoryが必要。
nativeは拡張子にかかわらずstampする。IDE側のVFS/doc epoch検査は引き続きEDTでI/Oせず行う。
これは同時外部writerに対するatomic filesystem transactionではなく、READY cacheの外部変更通知はIDE VFSへ依存する。

同pathでcompiler executableが更新されたときは、Sessionのreuse keyにsize・mtime（FileTime）・file keyを含め、旧常駐processを終了・reapして起動し直す。
request前後も同identityを照合する。これは信頼したcompiler自体の完全性を暗号学的に証明するものではない。
REDは未知closed dependencyの古いcandidateがREADYとなるassertionと、旧peerのresponseが新版更新後も返るassertion。
追加のknown native `.backend`同size内容変更fixtureは、VFS通知なしのpending responseを拒否して再解析する。

CIの全workspace回帰で既存SQLite Busy fixtureにも非決定的失敗を観測した。
0ms acquireはcheckout返却を待たず、rollback確認replyはcallback ownerのDropより先に送られる。
既存observerの`wait_returned(2)`を明示barrierとして用い、native Busy確認後のidle再取得を検査する。
取得timeoutの期待値・0ms契約・公開SQL API・runtime意味論は変えない。

navigationはphysical `.nagi`/`.low`上のlocals/functions/types/import先を優先する。
`stdlib:`仮想sourceへのdefinition jump、record field declarationの精密targetは未対応。
元sourceへ対応できないcompiler diagnosticは、messageを保ったfile-level表示にし、focus上のrangeを推測しない。

VS Codeは既存`symbols`/`check --editor-input`を使用する。新assistは同じoverlay schemaとsymbols payloadを再利用するが、
VS Codeのcompletion providerをこのPRで入れ替えてはいない。両IDEの候補機能が同等になったとは扱わず、VS Codeのassist移行は別PR候補。

性能目標はcache hitでprocess launchとdisk I/Oを増やさないこと、無効化されたsnapshotを再利用しないことを先に固定する。
release常駐protocolの数百msという観測をもとに350ms diagnostic/80ms completion debounceを設定したが、
入力から描画までの数値SLAはWindows GUI測定後に決める。これは性能より正確性を優先する境界である。

最終retry指摘は `05460bc`で解消しSol Highが再確認した。single workerのfinally後にEDT callbackを投入し、
未知dependencyに対する明示要求一回だけで自動再解析が完了するfixtureへ変更した。
最終4 targeted tests/buildPluginは成功。cache 500 lookupは初回mean約0.157 ms、最新全69件時は約0.133 ms、追加launch0（Platform fixture、実GUIではない）。
再回帰は99 result blocks/1012 passed/0 failed/1既存ignored、全workspace clippy/fmt成功。
WindowsのRust canonical verbatim drive/UNC pathとIDE VFS pathは同じsource identityへ正規化し、
navigationはcompiler targetをcanonical map経由で解決する。これはWindows実GUI試用の代用ではない。
