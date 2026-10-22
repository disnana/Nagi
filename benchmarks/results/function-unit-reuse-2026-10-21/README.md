# 関数依存単位の再利用: 実測と検証

未リリース。compiler入力は`39dc51d6c559e99523833a5a518e02b57f202e30`、baseはPR #139/#140反映後の`0e9a575`。同じreleaseビルドのcompiler（SHA-256はprovenance/raw参照）で、従来の`--profile dev --incremental`と新しい`--profile dev --incremental=units`を比較した。公開0.2.0や本番releaseの比較ではない。

## 条件と結果

Linux x86_64、共有host、実runtime、標準Cargo dev設定、同じNagi source path・seed Cargo.lock、各方式に独立した空target。timed buildとnative loopの間、他のローカルbuild/test/benchmarkは並走させていない。coldはregistry/OS cacheが温まった状態の1回で、物理disk coldではない。warmはstatsなし5回、body/APIは各1関数ずつ異なる5回の逐次編集の中央値。各成功exeの出力をその時点の全関数値と照合した。順序はbody→unitsで、無作為化や統計的有意性の検定はしていない。

| fixture・操作 | 本体1 crate | 最大8 units | 観測 |
|---|---:|---:|---|
| aggregate: cold | 37.678 s | 37.756 s | 同程度 |
| aggregate: 変更なし | 0.389 s | 0.385 s | 同程度 |
| aggregate: 関数内部編集 | 0.713 s | 0.662 s | 約7%短縮 |
| aggregate: 公開署名編集 | 0.722 s | 0.675 s | 約6%短縮 |
| print-each: cold | 39.975 s | 37.805 s | 依存ビルドの揺れを含む1回 |
| print-each: 変更なし | 0.404 s | 0.421 s | 約4%増 |
| print-each: 関数内部編集 | 0.883 s | 0.843 s | 約5%短縮 |
| print-each: 公開署名編集 | 0.880 s | 0.905 s | 約3%増 |
| print-each: 依存だけ温存した新app | 0.917 s | 0.965 s | 各1回、約5%増 |

両fixtureとも1,000個の小関数を使う。aggregateは別の集約関数から呼び、mainはその結果を1回表示する。print-eachはmainが全関数を直接呼び個別表示する。body編集ではaggregateが8中6単位、print-eachが8中7単位をCargo Freshとして保持した。実際にコンパイルしたartifactは従来2個に対し、それぞれ4個・3個。Rustソースが同じcallerでも依存manifestが変われば再コンパイルする。

frontendの完全検査・生成・公開処理は残り、Cargo起動前だけでもaggregateで約0.3秒かかる。小関数のcrate分割にはCargo/metadata/linkの追加費用があり、編集内容によって遅くなる。今回のopt-inを既定へ変更しない根拠でもある。すべてのアプリで高速、関数1個だけをnative compile、CI全体が同率短縮とは主張しない。

| 容量（計測終了時、MiB） | 本体1 crate | units |
|---|---:|---:|
| aggregate Cargo target | 1,433 | 1,435 |
| aggregate output | 440 | 483 |
| print-each Cargo target | 1,454 | 1,429 |
| print-each output | 439 | 476 |

outputは各成功世代のexe・snapshot・source CASを保持し、unitsではmanifestとsource mapも増える。自動GCはない。RSSはraw sampleの`max_child_rss_kib`を参照。wait4が返す最大child高水位で、並行processの合計メモリではない。source hit率とCargo Fresh率を区別し、unit別の記録を残した。

5,000万回のprimitive計算loopはdev実行7回の中央値が0.198→0.196秒（全checksum一致）。約1%の差から速度向上を結論しない。本番releaseは分割方式を変えず、production throughput、Rust/Axum・Python/FastAPI・Goとの比較は未測定。test/CI全体の時間・課金も、このローカルbuild実測から推定しない。

## 再現と証拠

```sh
cargo build --locked --release -p nagic
python3 scripts/benchmark_incremental_builds.py \
  --compiler target/release/nagic --baseline target/release/nagic \
  --baseline-mode body --current-mode units --fixture aggregate \
  --functions 1000 --repeat 5 --edit-repeat 5 \
  --work-dir local-only/function-unit-repeat --output local-only/function-unit-repeat/aggregate-results.json
```

print-eachは`--fixture print-each`へ変更する。ambientなprofile/Rustflags/wrapper上書きはharnessが拒否する。同じ独立empty targetを毎回新規作成し、既存利用者cacheをcleanしない。

- `aggregate.raw.json.gz` / `print-each.raw.json.gz`: 全sample・stats・unit全関数名を含む原記録。gzip解凍後のhashも`provenance.json`に記録。
- `summary.json`: 原記録から長い関数名リストだけを件数＋hashへ縮約した数値記録。原記録の代わりにnative reuseを認可しない。
- `cpu-results.json` / `dependency-warm-results.json`: dev native loopと依存だけが温まった新appの実測。`cpu_probe.py` / `cached_dependencies_probe.py`は使用したowned probe。
- `http-results.json` / `http_units_probe.py`: tutorial HTTPのowned copyへpure helperを追加し、実socketのGET 200/不正入力400、SIGINT正常終了、schema 3閉包検証を確認した。一般的なHTTP負荷試験ではない。

容量計測後、今回専有作成したaggregate両targetとprint-each baseline targetは削除した。計測途中の条件は変更せず、原記録・source・CAS・公開exeを保持している。古いtaskの再生成可能なCargo cacheも容量確保のため削除した。ネイティブcacheの長期保存・cross-checkout移植性の保証は追加していない。

## 検証

source段階でgeneration 73件、diagnostic 33件、CLI/project 13件、receipt 36件（LinuxではWindows固有1 skip）、release tools 154件、fmt/Clippy、日英Docs 102ページのリンク・anchor・assetを確認した。新規の安全性検査は成功時の出力だけでなく、孤児Cargo、子CAS追加入力、projection alias、Cargo/receipt/projection/latest失敗、旧exe byte/実行、両rootのlocked buildを含む。fixtureのSerde不足と禁止関数field、owned HTTP probeのstderr読取りミスはfixture側として修正し、言語契約やassertを緩めていない。

実Linux archiveはcheckout外の同梱compiler/runtime/toolsでexit 0（166.742秒、全workspace並走のため速度比較には使わない）。schema 1/2/3・独立unit Fresh・旧exe保持のmarkerを原logへ保存した。代表10 applicationのHigh/Low 19 native runsとCI policy 149件・利用者CI generatorも成功。seeded fuzzも1,000 mutations、16 bounded native、77 checked Low emission、panics 0で成功。core `39dc51d`の全workspace `cargo test --locked`もexit 0（831.467秒、他検証並走あり）。canonical shadowの追加testは別途native成功を確認し、final fmt/Clippyも成功した。最終独立監査と最新HEADの4 OS/配布CIは別gate。最終状態は本記録の追記とPR Checksで確認する。mock配布testや独立reviewを実archive/native検証の代わりにしない。版更新・tag・正式公開・mergeは行わない。
