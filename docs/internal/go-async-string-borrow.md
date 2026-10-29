# Go backend: read-only string borrowの最小段階

この段階は、共有checkerが受理した`view[str]`を同期関数とcold user asyncの入力、
local alias、同一identityのlocal async callbackへ接続する。Rust backendを既定のまま
維持し、Goでは`view[str]`をimmutableなGo `string` valueとして表現する。

## 保つ意味論

- `view(owner)`はpayloadを複製せず、Go string headerを値として渡す。`*string`、unsafe、
  await時のcaller binding再読込は使わない。
- 引数は左から一度だけ評価し、cold Future作成時のparameter値をclosureへ保持する。
  async本体はawaitまで開始しない。
- ownerのmove・上書き、viewのescape、branch/loopのorigin統合は共有checkerを正とする。
  Go emitterで名前やGC挙動から借用可否を再判定しない。
- `len`、`print`、`write`、`parse_i64`、検査済み比較と、既存の`Result` / `try await`
  を使える。panic、業務Err、toolchain失敗を同じ失敗へまとめない。
- callback identityはcheckerが封印した`FutureCapture`を使う。import aliasや同名shadowを
  文字列推測で解決しない。

## 意図的に拒否する範囲

`view[bytes]`、他scalar/class/resourceのview、`List[view[str]]`等のaggregate、view return、
`copy`、`slice`、借用sequenceのindex/iteration、native adapter、callback引数・戻り値として
外へ運ぶfunction signatureは未対応とする。対応しない型をGo `string`やsliceへ暗黙変換せず、
Go toolchain起動前の元source位置付き診断にする。

`FinalCheckFacts::FutureArgument.owners`は構文上の名前useとBindingIdのまま維持する。
0.3.0の後続facts段階では、ユーザーasync呼出しの各引数にtarget-neutralな
`UserAsyncArgumentFacts`を追加する。引数のexpr/demand/consume/hold成功直後、次の引数を
検査する前に、call/argument slot、Copy/Move、depthごとのtransitiveな`FlowOrigin`と
temporary borrow marker、`views_absent`を記録し、呼出し全体の成功後だけ公開する。
後の引数のmutationは先のsnapshotへ逆流させず、loop/recheckでは最終passで置換する。
sealはcheckerの再構築結果との完全一致を要求し、Lowへfactsをserializeしない。

G3aでは各引数へ`CaptureStorageId { call: ExprUseId, argument: slot }`を追加する。
これはsource ownerのidentityではなく、final AST上の呼出しと引数位置に対応するsynthetic
storage identityである。named owner、field、parameter、temporary、nested originでも同じ
規則を使う。Lowへserializeせず、各finalizeで再構築し、seal時のchecker再構築結果との
完全一致を要求する。loop/recheckも最終passのfactsだけを封印する。

releaseはcheckerだけが分類し、Go emitterはsealed planを検査してstorage IDに対応する
型付きtemporaryへ引数を左から一度だけ評価する。欠落・不整合はICE、`Deferred`は明示拒否とする。

- `NotRequired`: plain value graphのうちviewを含まないCopy値。
- `FutureInvocationExit`: 現対応のowned scalar/string/bytes、値class、payloadなしenum、
  List/Option/Resultのplain graphと、直接のread-only `view[str]`入力。
- `Deferred`: resource（Copy resourceも含む）、shared/task/authのpayload、一般viewや
  view aggregate、function/Future payload、再帰上限を超える等の証明不能なgraph。

従来の「viewなしCopyならNotRequired」はCopy resourceにも該当するため、G3aではplain
value graphの条件を加えた。resource metadataのCopyはcleanup不要の証明ではない。
originが空、または`views_absent`が真でもrelease可能とは推定しない。標準asyncとlegacy
builtin Futureは`UnplannedStandard`のままで、`sleep`にもuser argument planを付けない。

await開始時にbody closureをFuture objectからdetachし、active invocationのlocal参照は
正常return、Result Err、panic unwindの全出口で`defer`によりnilへ戻す。二度目のawaitは
本文を再実行せず拒否する。この境界はactive invocationによるcapture参照の保持終了だけを
意味し、Go GCのphysical memory即時回収、返却値・caller等の別参照の解放、resource close、
取消cleanup、Rustと同じretention時刻は保証しない。mutable sequence、resource view、
owner retirement、一般cleanup、task/scope/spawn/cancellation/timeout/実join/並行Future、
HTTP/JSON/DB/auth/WebSocket、公開Actor/Supervisor、Go native adapterは未対応のまま維持する。

## 検証

High、Highから保存したLow、独立した手書きLowを実Go toolchainでbuild/runし、named owner、
local viewの同一owner再代入、即時temporary、nested await、`Result` / `try await`、local callback、
import aliasのraw出力一致を要求する。生成Goはborrow入力とcallbackをGo `string` valueで保持し、
`*string`を生成しない。

negativeはowner借用中の上書きを共有checkerで拒否し、`view[bytes]` callback、view return、
view aggregate、`copy(view)`をGo toolchain欠損時にも元位置で拒否する。失敗時に新しいapp世代を
残さない。専用integrationの必須件数はLinux/macOS 34、Windows 33（既存のnamed callとsleepを含む）。Windowsだけ少ない1件は
既存のUnix symlink保護ケースであり、borrow検査のskipではない。

G3aは既存cold async/string borrowケース内でplain class/Listのowned capture、field borrow、
callbackとnested await、左から右の一回評価、Result tryを追加する。生成Goのnative testは
正常/Err/panic全てで本文開始前のdetachと二回目await拒否を観測する。invocation localの
`defer`は生成sourceも確認し、GC finalizerやmemory測定をreleaseのoracleにしない。
