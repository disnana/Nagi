# Go backend: ローカルasync callback identity

この段階は、ユーザー定義async関数を直接awaitする既存の最小実装に、同じ関数を
ローカル変数へ保持して呼び出す経路を追加する。標準operation、task、scope、spawn、
actor、HTTPは対象外で、Go toolchain起動前の明示的な拒否を維持する。

```nagi
async def increment(value: i64) -> Result[i64, Error]:
    return ok(value + 1)

async def main() -> Result[unit, Error]:
    callback = increment
    answer = try await callback(41)
    return ok(print(answer))
```

## 封印するidentity

共有checkerはFutureを返す各呼出しについて、実際のcallee、名前解決種別、引数の型・
passing・owner identityを`FinalCheckFacts`へ封印する。Go emitterはこのうちcallee identityを
使って到達関数を決め、ローカルcallback呼出しが実在するasync関数へ対応することを検証する。
ローカル変数の綴り、Rust生成名、同名関数の探索からcalleeを推測しない。

callback値はGoの`func(...)*nagiFuture[T]`へ落とす。callback作成でasync本体を開始せず、
呼出し引数は呼出し側で左から一度評価し、返されたcold Futureは従来どおりawaitで一度だけ
消費する。同じasync関数の再代入は許可し、別のasync関数への再代入は共有checkerの既存規則で
拒否する。

## 対象外

- callbackを関数引数、戻り値、class field、List、Optionなどへ保存すること
- read-only `view[str]`以外のborrowを含むcallback引数と、owner cleanup
- 複数Futureの並行実行、取消、timeout、scope join
- async標準operation、public actor、HTTP、DB、auth/session

GoのGCが値を保持することをNagiのborrow lifetimeやcleanup成功の代わりにしない。これらは
後続sliceでcheckerのowner factsとprivate lifecycle kernelへ接続してから対応する。

## 検証

High、Highから保存したLow、独立した手書きLow、import aliasを実Go toolchainでbuild/runし、
同じraw出力を要求する。同じcallbackの再代入と`Result` / `try await`を含める。異なるasync関数
への再代入はGo toolchainを欠損させた状態でも元source位置のchecker診断となることを確認する。

後続sliceではread-only `view[str]`入力を値として保持するlocal callbackを追加済みである。
その他のborrowとowner cleanupは引き続き対象外である。

この1件を加えた専用integrationの必須件数はLinux/macOS 30、Windows 29。Windowsだけ少ない
1件は既存のUnix symlink保護ケースであり、callback検査のskipではない。
