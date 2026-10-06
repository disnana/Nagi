# 値・task仕様の移行前検証

2026-10-06。main `e7aff1da0a36503d239d70cf5dbcf892655978e0` の動作を確認した。新しいmove operationやTask型の実装検査ではない。入力はSolの読み取り監査から作成し、rootがCLIをビルドして実行した。

## 実行条件と結果

- Linux、既存Rust toolchainとoffline Cargo cache。debug版nagicから生成アプリは既存release設定でbuild/runした。
- コンパイラSHA-256: `9bc1dd95cdca288b6356dfc17094b9590059d35778e5d4f0623eaadcd91f7bad`。
- 既存7 suite・36件が成功。コマンド・source tree・raw log hashは[実装計画](value-task-implementation-plan.md#監査の検証と現時点の停止条件)を参照。
- 下記9例×High/手書きLowの18入力と、6つの生成保存Lowをcheckした。受理18、期待した拒否6。正常6例をHigh/保存Low/手書きLowでbuild/runし、18実行の出力が一致した。
- CLIコマンドは48回（check 24、lower 6、buildを伴うrun 18）。42回がexit 0、拒否6回がexit 1。拒否理由・元行、正常stdout、stderrに出る成功artifact pathの存在を検査した。
- hidden clone、allocation数、任意の破棄順、4 OS、候補APIはこのprobeでは検証していない。Copy resource/ownedの全組合せは既存harnessと後続V1の検査へ分ける。

最初のprobe runnerはnative pathがstdoutに出ると誤認した。実際にはstderrで、全18アプリのstdoutは初回から期待と一致していた。失敗を成功に書き換えず、初回summary/logを保存し、streamの観測を修正して全48コマンドを再実行した。Nagiコード・出力期待・拒否期待は変えていない。

## 再実行

下のHighを`case.nagi`、手書きLowを`case.low`へ保存する。入力と生成先は別directoryにする。正常例では次の順で実行する。

```sh
nagic check case.nagi --no-project --out build/probe-high
nagic lower case.nagi --no-project --out build/probe-high
nagic check build/probe-high/generated.low --no-project --out build/probe-saved
nagic run build/probe-high/generated.low --no-project --out build/probe-saved
nagic run case.nagi --no-project --out build/probe-high
nagic check case.low --no-project --out build/probe-low
nagic run case.low --no-project --out build/probe-low
```

負例は両入力のcheckがexit 1になり、下記の行と理由を示す。生成Rustの失敗に遅らせない。

## 01-local-reinit

High

```nagi
def main():
    original = "Nagi"
    transferred = original
    original = "new"
    print(transferred)
    print(original)
```

独立した手書きLow

```nagi
fn main() -> unit {
    let original: str = "Nagi";
    let transferred: str = original;
    original = "new";
    print(transferred);
    print(original);
}
```

観測したstdout:

```text
Nagi
new
```

## 02-copy-matrix

High

```nagi
class Point:
    x: i64
enum Count:
    Value(value: i64)
    Empty
def add(value: i64) -> i64:
    return value + 1
def main():
    point = Point(x=7)
    point_alias = point
    print(point.x + point_alias.x)
    count = Count.Value(3)
    count_alias = count
    match count:
        case Count.Value(value):
            print(value)
        case Count.Empty:
            print(0)
    match count_alias:
        case Count.Value(value):
            print(value)
        case Count.Empty:
            print(0)
    nullable: i64? = some(9)
    nullable_alias = nullable
    match nullable:
        case Some(value):
            print(value)
        case None:
            print(0)
    match nullable_alias:
        case Some(value):
            print(value)
        case None:
            print(0)
    text = "Nagi"
    part = view(text)
    part_alias = part
    print(part)
    print(part_alias)
    callback = add
    callback_alias = callback
    print(callback(1) + callback_alias(1))
```

独立した手書きLow

```nagi
record Point { x: i64; }
enum Count { Value(value: i64); Empty; }
fn add(value: i64) -> i64 { return value + 1; }
fn main() -> unit {
    let point: Point = Point(x=7);
    let point_alias: Point = point;
    print(point.x + point_alias.x);
    let count: Count = Count.Value(3);
    let count_alias: Count = count;
    match count { case Count.Value(value) { print(value); } case Count.Empty { print(0); } }
    match count_alias { case Count.Value(value) { print(value); } case Count.Empty { print(0); } }
    let nullable: Option[i64] = some(9);
    let nullable_alias: Option[i64] = nullable;
    match nullable { case Some(value) { print(value); } case None { print(0); } }
    match nullable_alias { case Some(value) { print(value); } case None { print(0); } }
    let text: str = "Nagi";
    let part: view[str] = view(text);
    let part_alias: view[str] = part;
    print(part);
    print(part_alias);
    let callback: fn[i64, i64] = add;
    let callback_alias: fn[i64, i64] = callback;
    print(callback(1) + callback_alias(1));
}
```

観測したstdout:

```text
14
3
3
9
9
Nagi
Nagi
4
```

## 03-move-identifiers

High

```nagi
def move(text: str) -> str:
    return text
def main():
    payload = "user function"
    transferred = move(payload)
    move = 7
    alias = move
    print(transferred)
    print(move + alias)
```

独立した手書きLow

```nagi
fn move(text: str) -> str { return text; }
fn main() -> unit {
    let payload: str = "user function";
    let transferred: str = move(payload);
    let move: i64 = 7;
    let alias: i64 = move;
    print(transferred);
    print(move + alias);
}
```

観測したstdout:

```text
user function
14
```

## 04-result-noncopy

High

```nagi
def main():
    result: Result[i64, i64] = ok(1)
    alias = result
    match result:
        case Ok(value):
            print(value)
        case Err(value):
            print(value)
```

独立した手書きLow

```nagi
fn main() -> unit {
    let result: Result[i64, i64] = ok(1);
    let alias: Result[i64, i64] = result;
    match result {
        case Ok(value) { print(value); }
        case Err(value) { print(value); }
    }
}
```

観測したcheck拒否: line 4、`result はmove後`。

## 05-borrowed-local

High

```nagi
def main():
    owner = "Nagi"
    borrowed = view(owner)
    alias = owner
    print(borrowed)
```

独立した手書きLow

```nagi
fn main() -> unit {
    let owner: str = "Nagi";
    let borrowed: view[str] = view(owner);
    let alias: str = owner;
    print(borrowed);
}
```

観測したcheck拒否: line 4、`viewまたはループから参照`。

## 06-async-alias

High

```nagi
async def answer(value: i64) -> i64:
    return value + 1
async def main():
    callback = answer
    alias = callback
    print(await callback(41))
    print(await alias(41))
```

独立した手書きLow

```nagi
async fn answer(value: i64) -> i64 { return value + 1; }
async fn main() -> unit {
    let callback: fn[i64, Future[i64]] = answer;
    let alias: fn[i64, Future[i64]] = callback;
    print(await callback(41));
    print(await alias(41));
}
```

観測したstdout:

```text
42
42
```

## 07-fresh-field

High

```nagi
class Pair:
    first: str
    second: str
def main():
    pair = Pair(first="first", second="second")
    first = pair.first
    items = [pair.second]
    result: Result[List[str], i64] = ok(items)
    match result:
        case Ok(values):
            print(first)
            print(len(values))
        case Err(number):
            print(number)
```

独立した手書きLow

```nagi
record Pair { first: str; second: str; }
fn main() -> unit {
    let pair: Pair = Pair(first="first", second="second");
    let first: str = pair.first;
    let items: List[str] = [pair.second];
    let result: Result[List[str], i64] = ok(items);
    match result {
        case Ok(values) { print(first); print(len(values)); }
        case Err(number) { print(number); }
    }
}
```

観測したstdout:

```text
first
1
```

## 08-shared-handle

High

```nagi
def main():
    handle = share("Nagi")
    duplicate = clone_shared(handle)
    transferred = handle
    print(handle)
```

独立した手書きLow

```nagi
fn main() -> unit {
    let handle: shared[str] = share("Nagi");
    let duplicate: shared[str] = clone_shared(handle);
    let transferred: shared[str] = handle;
    print(handle);
}
```

観測したcheck拒否: line 5、`handle はmove後`。

## 09-owning-views

High

```nagi
def pass_parts(parts: List[view[str]]) -> List[view[str]]:
    alias = parts
    return alias
def main():
    owner = "Nagi"
    parts = [view(owner)]
    alias = parts
    print(pass_parts(alias)[0])
    print(owner)
```

独立した手書きLow

```nagi
fn pass_parts(parts: List[view[str]]) -> List[view[str]] {
    let alias: List[view[str]] = parts;
    return alias;
}
fn main() -> unit {
    let owner: str = "Nagi";
    let parts: List[view[str]] = [view(owner)];
    let alias: List[view[str]] = parts;
    print(pass_parts(alias)[0]);
    print(owner);
}
```

観測したstdout:

```text
Nagi
Nagi
```

## 実行ログの照合情報

raw logsはこの作業環境の`/tmp/nagi-value-task-audit-proof/`、初回runnerの失敗は同directoryの`first-harness-run/`に保存した。Gitの文書差分には、再実行できる入力と次のhashを残す。

| コマンド | exit | raw log SHA-256 |
|---|---|---|
| 01-local-reinit-high-check | 0 | `bf6da1496d9782c27fa9ccc65ce943e39dfa7fd7f43042b95be5b9c15b1e2d79` |
| 01-local-reinit-high-lower | 0 | `a0dc3402c5f539d4a2fceb6ea2851196797129cab3c57c9151ea81b32101c309` |
| 01-local-reinit-high-saved-check | 0 | `a7755a2b77867b8982ba0f23db5a57e9ca2c2ac39532dd09e2bf4ddac7e60a2e` |
| 01-local-reinit-high-saved-run | 0 | `a4f6ff47e069e5813c9390306307330b9cac5bb147f9311933f6b2b275cb0edb` |
| 01-local-reinit-high-run | 0 | `438ba0dce86207c473f5109a8937a301630ab7c8f4198fd191bc6999a9314765` |
| 01-local-reinit-low-check | 0 | `e3789c7244a6f89e9f65963934505575a99952fd70a7f86c24b89beba558974b` |
| 01-local-reinit-low-run | 0 | `33d8e657d737a2b0264973dc4fe637a0a99c74b54eb3047d54fd504d300378f0` |
| 02-copy-matrix-high-check | 0 | `ce32bacb23053774d1fbd2dc72bc3e67e781d97a4d5b40fc0b61e903bd393b7f` |
| 02-copy-matrix-high-lower | 0 | `2af5cefa14691f42379155245e63b55d46d1d159d71d26a51b18adb453792e53` |
| 02-copy-matrix-high-saved-check | 0 | `302338d77c66012b550af0a64b8db889548c27785494efb4d781003ed1348625` |
| 02-copy-matrix-high-saved-run | 0 | `76f9566eee649b6cb26daf1bce8c3aeefb39db8404d4c7bdb06515bb86ea4d85` |
| 02-copy-matrix-high-run | 0 | `e8d3128b76e539667a512eaf7891896229f00b4174b586b495379d5f9cffe056` |
| 02-copy-matrix-low-check | 0 | `e0e2b7db7867fc675477c9fe61a121c756c95144cbc82134bc9df46d2af4a7f4` |
| 02-copy-matrix-low-run | 0 | `7f1852502578fad11a9444dd3c430b51930cd70fabc4e8d4c07b5b7ef09e4b83` |
| 03-move-identifiers-high-check | 0 | `a75e1384b3b37ac98066ae0565e39d65b3b589e7a36e3be8d53c538ae2eeb38c` |
| 03-move-identifiers-high-lower | 0 | `1eb538a6ebae7da28c44645f5cf14e70de5e8f76f43ea5a9e8997b66d3369b4c` |
| 03-move-identifiers-high-saved-check | 0 | `f1a63c885e988ac915a318dce493f97bef2423a2e19c8eb9ca36a3042eb6bc77` |
| 03-move-identifiers-high-saved-run | 0 | `e534a0fc05a6906c3148742fe2bb2e6ffb8f39bc7ef124f233cbad01a0a805a4` |
| 03-move-identifiers-high-run | 0 | `b0dd9a05e691cae9dc5cb6478f13897138cfd84c1f635048b1ddc065f532e42a` |
| 03-move-identifiers-low-check | 0 | `fd7b5046a89d9599fddeb5e7b1f44ae6eac052c6cb80b0490013b8d8edbf1d5b` |
| 03-move-identifiers-low-run | 0 | `2b0d32aeb34718a2f215a6bb5913e2d7a734a4631cf27524254ff84c464bee94` |
| 04-result-noncopy-high-check | 1 | `b76f823973d4d8573f0f9f88f6c15b8380c406e79a9605d8e490b0b6cdf7f43e` |
| 04-result-noncopy-low-check | 1 | `bfd6be8ea1e31778dc5d2a636f278162ccfad0bc5842229fb8878b52956657b6` |
| 05-borrowed-local-high-check | 1 | `150eab79ff6f8d3dd7bcb2a12160e6f3dbf1c56d42098dee4fe118181750900e` |
| 05-borrowed-local-low-check | 1 | `280456c9dcfdeb3f04c2551271074e522b0ad7debdeb5dfa31eb7b3f7dd8ff09` |
| 06-async-alias-high-check | 0 | `978ee804971716cffab164700baec209f19c0c068393597bfa32229950944255` |
| 06-async-alias-high-lower | 0 | `4db48b234bf2ec95da88be5a1aa2cecf16fce66cec5429df1a4b6584be9af621` |
| 06-async-alias-high-saved-check | 0 | `3175c8ef430303568c8b9cdb3cc0acab7c1f14dccade8c9436ff1cda7b805923` |
| 06-async-alias-high-saved-run | 0 | `25d66b4e0d82803ccdd6b43d78fc38ede286b4e751f49c700e1dfe5099ecb4b2` |
| 06-async-alias-high-run | 0 | `4c581f09a32e10e85a11d8248199b727616ea487f72a5ac7353a902d5f705bd1` |
| 06-async-alias-low-check | 0 | `d91c4499f37ceb332b70c331e1451c65648c080f221e73b8f441a649e1088c25` |
| 06-async-alias-low-run | 0 | `284267d4699e7bb7f3b08f4fd65e7e4028479a6ce34ffc594ea179fecf381233` |
| 07-fresh-field-high-check | 0 | `cac81c54c31703733008ad15dd47399639773f433d792945835577c64a6cc047` |
| 07-fresh-field-high-lower | 0 | `3111695fa166be2c417ec93a8cf651c8bd6911e11dbee0c6c46c43c0c84ad4ef` |
| 07-fresh-field-high-saved-check | 0 | `4301908e1694a14f9977023e16b4d0ddaf0a60cfab3a43a8dfd5a1019368caa9` |
| 07-fresh-field-high-saved-run | 0 | `e432cddfeec29891522385a22fc815b5dde0fcc6428b88cd9719f73dc4a73117` |
| 07-fresh-field-high-run | 0 | `143e8dab449395a4fdbdf75e360b3d5ad11ad02a34ad8d2166694e9772486e31` |
| 07-fresh-field-low-check | 0 | `d7ccd87dec269e640338fafa77571b95e7f0470aa36fe1216ab5545854a2dcfb` |
| 07-fresh-field-low-run | 0 | `5f4841a13c004f3b5f29d89a1d58b2d1557eaef831e533e4bb7407fca551c9d9` |
| 08-shared-handle-high-check | 1 | `968f52aa2d1a5f4d237fb2af526517156e87ad23cb0e608c3b9807c880737dac` |
| 08-shared-handle-low-check | 1 | `3ecb99c79aac0026622bb585afc1d3cfbe2b164fab3af883d593e163a74f6219` |
| 09-owning-views-high-check | 0 | `770e7fed91fdd033b1ccc382a2968c564edabe7cc01abf3187cd804b3e67b29d` |
| 09-owning-views-high-lower | 0 | `e48c39912f73fd24aa60d5baa9e0efd3167e25a832cab1915f5eef2f85d16bf0` |
| 09-owning-views-high-saved-check | 0 | `8c4ea0ce98f8b303125553f231d7b7d52a8d3e2e39d2861555e43e3e1e067830` |
| 09-owning-views-high-saved-run | 0 | `fa5615431b3dfa479bd4967aec255f164db31288d90ce79cd799e139a20b6f39` |
| 09-owning-views-high-run | 0 | `762095a6d90e315aca5e4c728be67ef407b85bd138265f94bfac20201fa7d684` |
| 09-owning-views-low-check | 0 | `33d1ac2116d647833681c8e0d385768c57643a17c9a523f1fc2c6fe5ffbd3116` |
| 09-owning-views-low-run | 0 | `6ca3b734b79968e1af9ce2fb59a0fd61e57964ff501ec01ad6cef99f8ada318b` |

この検証で新たなcheck/build反例は見つからなかった。有限の小例についての結果であり、所有権モデル全体の正しさの証明ではない。
