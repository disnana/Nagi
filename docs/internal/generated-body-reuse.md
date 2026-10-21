# 生成Rust本体の内容アドレス方式再利用（開発ソース）

対象baseはPR #138のmain反映`1386411565eeb8b0c00772441395b48e06104b55`。ユーザーの「差分データを活用して最低限に抑え込めないのか」に対する第2差分。0.2.0互換を維持し、未リリースの`build/run --profile dev --incremental`だけへ追加する。実装前のSol 6.1 High reviewで以下の制約を確認した。

## 再利用の粒度

完全High/Low/Policy/ownership検査とsealed emission planから、毎回Rustを生成する。本体は本物のCargo libraryとしてcrate rootへ置き、synthetic mainだけを明示的に`pub fn main`として生成する。同じ内容は単独binとしても有効で、従来の互換projectionとsnapshotの`src/main.rs`を維持する。ユーザーコードの文字列置換やnested `include!`を使わない。

`generated_body.rs`は`nagi-generated-body-v1\0`をdomainとし、body manifest、Rust sourceの順にそれぞれのbyte長（u64 big endian）と全byteをSHA-256へ入れる。manifestにはschemaで固定するpackage/lib設定とcanonical runtime pathが入る。このhashは**生成ソースの同一性**であり、native cache keyではない。型・関数hashは引き続き変更理由の観測に使い、未知の依存edgeを理由に検査を省略しない。

本体を`out/.nagi/apps/<app>/code/<hash>/`へ置き、世代固有binの`src/entry.rs`が`nagi_app_body::main()`を呼ぶ。Cargoを毎回実行し、同じ本体のnative成果物と依存をCargoに再利用させる。本体変更では一つのlibrary crateを再コンパイルする。Rustのmonomorphizationやリンクの粒度を、Nagiの関数差分粒度と混同しない。

## 互換性と整合性

- 既定release、通常のdev、公開Rust embedding APIは従来の生成を維持する。最適化profileやoverflow、panic/unwindを変更しない。
- Rust adapter、追加Rust依存、managed retentionは既存monolithic構成へfallbackし、理由を表示する。任意Rustの`crate::`、Cargo環境macro、生成entry guardのlifecycleを勝手に変更しない。
- generationとcompatibility manifestはdirect runtimeと同じbody依存を持つ。両方で継承lockの`cargo build --locked`を検査する。CAS libraryを自動workspace memberにしないよう、root workspaceでbody pathをexcludeする。
- incrementalの両rootとCASは、prepare時に確定した同一のcanonical runtime絶対pathを使う。Cargoのpath sourceは文字列の別名を別packageとして扱うため、rootを別名で開くmacOSの`/var`／`/private/var`やruntime子のsymlinkでも二重のruntime identityを作らない。既定・fallbackのmanifest生成には適用しない。
- outの既存OS lockの下で専有stagingを作り、完成したsource cacheをrenameで公開する。hitは今回の生成manifest/Rustとbyte単位で照合し、破損は修復せず失敗。親directoryとfileのlink/reparse・種類を検査し、入力identityも保護する。
- 本体packageはbuild scriptとbin/example/test/benchの自動探索を無効にする。CASのrootは`Cargo.toml`と`src`、`src`は`lib.rs`だけに限定し、未知の名前・追加file/directory・読み取り失敗を拒否する。Python receipt検証も同じ形を照合する。既存entryへ追加した`build.rs`が以前は実行される再現を確認し、追加入力の実行前拒否・旧latest/実行file保持を回帰検査する。このmanifest変更は内容キーにも入る。
- 通常check/lower/buildを含むprojection writerは、既知の全appの保持本体へのaliasも入力identityとして保護する。lock待機後とCargo完了後のprojection時にも再検査する。CASへhardlinkしたprojectionから旧本体を書き換えない。
- 新しいbin名、immutable exe copy、旧latest保持、publish→projection→latest、run前unlock、孤児Cargoの世代分離はADR007を維持する。CASは自動回収しない。

## 診断とreceipt

Cargo targetの`src_path`が本体libraryと一致する場合だけ、その`src/lib.rs`のspanを現在の生成provenanceへ写す。shimと別依存の診断は元のRust診断を保持する。相対path、Windows prefix、High/import・保存Lowを検査する。

schema 2 receiptは既存固定成果物と`src/entry.rs`、`body-Cargo.toml`、body digestを持つ。Python 3.10対応検証器は固定のtable/key形だけを受け入れ、任意のTOML parserは実装しない。snapshotから内容キーを再計算し、既知のapp祖先から導いたCASだけを読み、内容と生成依存参照を照合する。schema 1は維持する。完全なnative入力の証明・署名ではない。外部の悪意ある同時directory差し替え、Cargo cacheの改変、電源断耐久は追加保証しない。

Windowsでもsynthetic body参照は`/`区切りで固定する。root manifestの任意runtime pathは一行のTOML basic stringへescapeし、JSON文字列との字句差であるDELも明示escapeする。独立auditで見つけた改行・DELのP2を修正し、実Cargoで改行／quote／DEL pathを使う回帰とPython検証器で再確認した。one-line literal stringを使う従来のschema 2も受け入れる。

## 検証

既存generation/shared-target/output-safety/dependency/diagnostic harnessを拡張し、同一・コメント・本体・署名・runtime変更、High/保存Low、sync/async/Result/no-main、fallback、cache破損・link/junction、projection alias、孤児Cargo、Cargo/receipt/projection/latest失敗を観測する。配布gateはcheckout外でschema 1/2、2回の本体再利用、四つの旧exe保持も実行する。

再現用は`scripts/benchmark_incremental_builds.py`。通常devと同じprofileのincrementalを、実runtime、同じNagi source path、同じseed lock、別の空targetから比較する。coldはregistry/OS cacheが温まった状態。観測あり・なしのwarm、コメント・本体・公開署名変更を区別し、native出力も確認する。CI全体や一般的な実行速度の改善とは主張しない。

[実測とraw結果](../../benchmarks/results/generated-body-reuse-2026-10-21/README.md)では、1,000関数fixtureの変更なしbuildはstatsなし0.713→0.398秒、statsあり0.819→0.490秒。coldはほぼ同じ。本体変更時にlibrary全体を再コンパイルする制約も同じ表へ残す。
