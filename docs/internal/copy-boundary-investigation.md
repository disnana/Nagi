# Copy判定の深さとRust deriveの差

2026-10-05。Phase 3の基点`27c8bf4`で、登録資源の集約とは別に調べた。今回の資源metadata変更では、この受理・拒否やderiveを変更しない。

## 確認したこと

`C0 → C1 → … → C(n-1) → leaf`のclass field列を作り、各classは一つの`value` fieldだけを持たせた。leafは`i64`、`owned[i64]`、`Option[i64]`、`std.http.server.Status`の4種類、class数は63・64・65の3種類。

別々の二つのプログラムを検査した。

1. `twice(value: C0)`から`consume(value)`を2回呼ぶNagiをcheckする。
2. 同じclass列を持ち、unitのRust adapterを呼ぶNagiをcheck/buildする。手書きadapterだけで`demand<T: Copy>()`に`C0`を渡し、生成された型のCopy実装をrustcへ問い合わせる。

| leaf | class数 | 2回使用のcheck | adapterのCopy要求を含むbuild |
|---|---:|---|---|
| i64 | 63 / 64 | 成功 / 成功 | 成功 / 成功 |
| i64 | 65 | move後使用として拒否 | 成功 |
| owned[i64] | 63 | 成功 | 成功 |
| owned[i64] | 64 | move後使用として拒否 | 成功 |
| owned[i64] | 65 | move後使用として拒否 | 手書きadapterのE0277 |
| Option[i64] | 63 | 成功 | 成功 |
| Option[i64] | 64 | move後使用として拒否 | 成功 |
| Option[i64] | 65 | move後使用として拒否 | 手書きadapterのE0277 |
| http.Status | 63 / 64 | 成功 / 成功 | 成功 / 成功 |
| http.Status | 65 | move後使用として拒否 | 成功 |

12通りすべてでadapter呼出しを含むNagiのcheckは成功した。E0277になった2通りは、同じNagiのままCopy要求のない別adapterを使うとbuildが成功した。失敗箇所は手書きRustのtrait要求で、Nagiだけの型・所有権・lifetimeによるbackend rejectionとは分類しない。生成Rustをpatchして通した結果ではない。

## 共通原因と分類

P2: checkerのCopy判定と、封印時のderive判定に別の再帰起点がある。

- [`Checker::copy_type`](../../compiler/src/check.rs)は型全体からdepth 0で開始し、登録資源の判定より先に`depth > 64`を検査する。
- [`check::checked`](../../compiler/src/check/checked.rs)のderive判定はclassの各fieldから開始する。登録資源の判定と深さ検査の順序もcheckerと異なる。

この有限probeで見つかったのは保守的な拒否と生成traitの差。unsoundness、受理されたNagiだけのbuild失敗、余分なcloneやDropの変化は確認していない。深さ以外の全Copy判定が一致する証拠にはならない。

## 再現用の小さい生成器

次のPythonでclass列と2回使用の例を作れる。`depth`と`leaf`を上の表に合わせて変更し、通常の`nagic check --no-project`で検査する。

```python
from pathlib import Path

depth, leaf = 65, "i64"
prefix = "import std.http.server as http\n" if leaf == "http.Status" else ""
classes = prefix + "".join(
    f"class C{i}:\n    value: "
    + (f"C{i + 1}" if i + 1 < depth else leaf) + "\n"
    for i in range(depth)
)
Path("twice.nagi").write_text(classes + """def consume(value: C0):
    return
def twice(value: C0):
    consume(value)
    consume(value)
def main():
    return
""", encoding="utf-8")
Path("probe.nagi").write_text(classes + """@rust("native::probe")
extern def probe() -> unit
def main():
    probe()
""", encoding="utf-8")
Path("native.rs").write_text(
    "pub fn probe() { fn demand<T: Copy>() {} demand::<super::C0>(); }\n",
    encoding="utf-8",
)
```

`nagic build probe.nagi --rust native.rs --no-project`で生成traitを検査する。Copy要求を外す比較では、元adapterを保存したまま、別fileに`pub fn probe() {}`を書いて別の`--out`へbuildする。通常のサポート環境のRust/Cargo/runtime依存が必要。

## 次の判断

同じ型queryをcheckerと封印側で使う必要がある。ただし、深さ起点だけを揃えると、現在Rustに公開されるCopy deriveが減る可能性がある。cycleを区別する有限graph queryなら深いCopy型の受理を広げる可能性がある。深さ制限を公開の未対応診断にする案も、現行の利用者契約を決め直す。

推奨する次の作業は、共通queryの設計と両方向のpass/fail・Rust trait・診断の比較を先に作ること。受理規則またはinterop traitを変更する場合はStop条件に沿って判断する。現在の差を正常golden、backend拒否allowlist、skipとして固定しない。修正時にはこの生成器から縮小した恒久回帰を追加する。

実行時のsource・adapter・stdout/stderr・exit statusは作業環境の`/workspace/test-tools/compiler-rust-boundary-plan/copy-depth-probes/`へ保存した。これはリポジトリの常設テストではない。
