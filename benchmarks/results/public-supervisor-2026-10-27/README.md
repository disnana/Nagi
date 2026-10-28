# 公開Nagi Supervisor: 2026-10-27の有限実測

Rust backend 0.2.1の実Nagiアプリを最適化releaseで実行した。裸Tokio actorの模擬ではない。
[再現と集計の定義](../../public-supervisor/README.md) · [契約・隔離限界](../../../docs/internal/public-supervisor-validation.md)

全24case/72phaseが有限guard内で終了し、2,880,000正常返信、正常呼出の失敗0。
下表は各CPU/actor群の3round×正常前後2phaseの中央値。1actor群は同じactorに2送信者、他は別actor。
故障は18caseのみ。CPU単価には同processの送信者と観測overheadを含む。

| CPU affinity | actors | idle RSS MiB | idle増分 KiB/actor | 正常成功/s | CPU ns/成功 | p99 µs | peak RSS MiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 1 | 2.035 | 168.00 | 1,448,886 | 666 | 3.28 | 8.496 |
| 1 | 32 | 2.125 | 9.88 | 1,695,536 | 547 | 2.39 | 8.840 |
| 1 | 128 | 2.508 | 5.53 | 1,727,575 | 565 | 2.21 | 9.207 |
| 1 | 512 | 4.035 | 4.44 | 1,362,544 | 697 | 2.88 | 10.652 |
| 2 | 1 | 2.000 | 172.00 | 988,003 | 1574 | 11.10 | 13.039 |
| 2 | 32 | 2.211 | 10.25 | 1,953,762 | 937 | 1.52 | 9.723 |
| 2 | 128 | 2.535 | 5.69 | 1,263,640 | 1372 | 6.92 | 12.418 |
| 2 | 512 | 4.070 | 4.48 | 1,904,276 | 983 | 1.45 | 11.547 |

512actorのidle RSSは約4MiB、増分/actorは固定Supervisor等を含む。これは最大収容数ではない。
2CPUの数値は変動し、単純なincrement handlerが必ず並列化で速くなる結果ではない。
全phaseの最小common-active Jain indexは0.916。一般的公平性や無飢餓は保証しない。

| CPU affinity | actors | 故障中の正常完了/interval 中央値 | 同成功/s 中央値 | 同p99 µs 中央値 |
| --- | --- | --- | --- | --- |
| 1 | 32 | 13,722 | 1,320,837 | 2.41 |
| 1 | 128 | 17,178 | 1,642,952 | 2.54 |
| 1 | 512 | 13,434 | 1,247,667 | 2.90 |
| 2 | 32 | 22,458 | 2,208,571 | 1.17 |
| 2 | 128 | 17,717 | 1,625,610 | 1.53 |
| 2 | 512 | 23,044 | 2,107,751 | 1.19 |

故障区間は失敗call直前から公開generation2イベント・state reset応答まで（既定10ms delayを含む）。
正常actorの故障区間内完了はすべて成功。故障対象自身の意図したREPLY_LOSTは別枠で成功に数えない。
全caseの最終FD差0、Tokio task差0。RSSにはallocator/event storage/最後の40,000返信の観測vectorが残り、
最終RSS増分をactor leakや全回収の証明とは解釈しない。短時間で長期安定性は断定できない。

`aggregate.json`には中央値だけでなく各群のmin/maxを保持する。`summary.json`は24caseのcheckpointとphase、
`*.jsonl.gz`はすべての個別roundtripと故障時刻。72raw phaseを集計関数へ戻して完全一致を確認した。
`provenance.json`はsource/生成Rust/実binary/toolchain/tree、`native-Cargo.lock`はTokio1.54.1を含む実依存解決。
`native-Cargo.toml`はpathを含む証拠であり単独でportable projectではない。
build.logは初回4m08s、後の2buildは起動準備観測と回数上限更新を反映したwarm buildで、速度比較ではない。
contractログはHigh/保存Low/手書きLowのnative結果と期待panic、既存harnessの成功summaryを保存する。
Linux x86_64のみ実行。4OS新回帰・exact-head CI・別担当の独立最終reviewは別gate。

検証logの末尾の余分な空行だけを正規化し、元bytesのSHAをprovenance.jsonに保存した。raw gzipは未変更。
