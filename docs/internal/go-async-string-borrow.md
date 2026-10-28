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

`FinalCheckFacts::FutureArgument.owners`は構文上の名前useとBindingIdであり、transitiveな
borrow origin証明ではない。alias先owner、field path、static origin、temporary loan、
content depthを表さない。immutable string入力はoriginから生成動作を決める必要がないため
既存factsで実装できるが、mutable sequence、resource view、owner retirement、決定的cleanupへ
広げる前にtarget-neutralなorigin/storage/release planを封印する必要がある。

Go GCによるphysical memory回収時刻やRustと同じretentionを保証しない。Futureが消費時に
body closureを解放する既存契約は維持するが、これを一般owner cleanupの実装証拠にしない。

## 検証

High、Highから保存したLow、独立した手書きLowを実Go toolchainでbuild/runし、named owner、
local viewの同一owner再代入、即時temporary、nested await、`Result` / `try await`、local callback、
import aliasのraw出力一致を要求する。生成Goはborrow入力とcallbackをGo `string` valueで保持し、
`*string`を生成しない。

negativeはowner借用中の上書きを共有checkerで拒否し、`view[bytes]` callback、view return、
view aggregate、`copy(view)`をGo toolchain欠損時にも元位置で拒否する。失敗時に新しいapp世代を
残さない。専用integrationの必須件数はLinux/macOS 32、Windows 31。Windowsだけ少ない1件は
既存のUnix symlink保護ケースであり、borrow検査のskipではない。
