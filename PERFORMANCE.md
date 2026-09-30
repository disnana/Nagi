# Nagi 0.1 性能・検証報告

この版は、Rust製コンパイラと実ランタイムを持つ、二層バックエンド言語の試作です。Highから編集可能なLowテキストを生成し、それを再解析・検査してRust経由でネイティブへコンパイルします。HTTP body → 型付きclass → SQLite → class → JSONのCRUD経路、Lowの通常関数呼び出し・関数置換、async/scope、actor間通信、worker再起動が実際に動きます。

汎用言語、本番用独自runtime、BEAM相当の障害隔離は未完成です。actor・Supervisor・queueは実ランタイムを呼ぶ標準試験関数で、Highの汎用宣言構文はまだありません。以下の数値は、記載した測定環境で採取したログに基づきます。サンプル名は公開用に匿名化し、測定時と同じASCIIのバイト長を維持しています。測定値は変更していません。

## 測定環境と再現条件

| 項目 | 今回の条件 |
| --- | --- |
| OS | Linux-6.18.44-x86_64-with-glibc2.39 |
| CPU | INTEL(R) XEON(R) PLATINUM 8573C |
| CPU割当 | affinity 0–8、cgroup quota 8 CPU。物理ホストの共有状況と周波数は制御していない |
| メモリ上限 | 8 GiB cgroup |
| Rust / Cargo | 1.98.1 (48a229cea 2026-09-01) / 1.98.1 |
| ビルド | release、opt-level=3、LTO=false、codegen-units=1、panic=unwind。target-cpu=nativeは使用しない |
| 主要依存 | Tokio 1.53.1、Axum 0.8.9、serde 1.0.229、serde_json 1.0.151、rusqlite 0.40.2（bundled SQLite）。Cargo.lockを同梱 |
| CPython / Node | 3.12.14 / v24.19.0 |
| HTTP Python | aiohttp 3.13.5 + slots dataclass |
| load generator | native wrk 4.2.0、commit a211dd5a7050b1f9e8a9870b95513060e72ac4a0 |
| 時刻 | 2026-09-30 UTC。詳細は environment.json |

CPU/microbenchはserver負荷・ビルドと重ねず、CPU 0へ固定して個別に実行しました。各Rust測定は5回warmup、pilotから1反復のloop数を決め、7反復のmedianです。Pythonは5回warmupと7×25回、NodeはJIT用50回warmupと7×100回です。準備済みの同じ100,000要素を使用し、配列生成は計測外です。整数入力は `(i*17+13)%997-498`、floatはその値を7で割ります。Node Numberでも今回の整数入力・結果は正確ですが、i64全範囲の意味は共有しません。

試行内にも揺れがあり、CPU周波数、共有ホストの競合、cache状態の完全な制御はしていません。単一環境での探索的計測です。別実行の数%の差から言語の優劣を断定しません。raw_nsを保存し、良い値だけの選択はしていません。

allocation counterは呼び出しthreadのRust GlobalAlloc要求を数えます。SQLiteのC allocator・他threadのjob/task allocationは含みません。reallocはalloc回数にも含め、byteは要求量の累計でlive/peak memoryではありません。戻り値のdropはcount取得後なので、そのdeallocationは表に入りません。時間計測中はcounterを無効にしますが、allocator転送時のTLS分岐は残ります。静的cost reportは発生箇所であり、動的allocation回数ではありません。

## CPU：同じ入力・同じ結果

kernel全体の時間はµs/opです。ns/itemは100,000要素で割ったamortized値であり、一つの加算の単独latencyではありません。

| kernel | Nagi µs/op | Rust µs/op | Python µs/op | Node µs/op | Nagi ns/item | Python/Nagi |
| --- | --- | --- | --- | --- | --- | --- |
| integer_sum | 14.667 | 13.895 | 2568.563 | 95.678 | 0.147 | 175.1× |
| map_reduce | 33.973 | 35.908 | 5622.608 | 136.259 | 0.340 | 165.5× |
| filter_reduce | 65.030 | 101.582 | 2317.947 | 139.674 | 0.650 | 35.6× |
| branch | 69.915 | 74.350 | 4514.531 | 271.324 | 0.699 | 64.6× |
| float_sum | 75.123 | 80.055 | 1637.475 | 79.255 | 0.751 | 21.8× |
| loop | 118.099 | 134.439 | 5173.244 | 193.316 | 1.181 | 43.8× |

checksumは4実装で一致しました（floatは許容誤差1e-7）。`{"integer_sum": -3184, "map_reduce": 90448, "filter_reduce": 12461527, "branch": 2106, "float_sum": -454.85714285709753, "loop": 299995}`。Nagi/Rustのkernel内Rust allocationは全て0回です。Python/Nodeのallocationは未計測で、この0と比較しません。

CPythonのbuiltin sumは別の参考値として 686.172 µs/opでした。表のPythonは明示ループであり、NumPy等のnative kernelとの比較ではありません。データは反復中cacheへ温まるため、DRAM帯域や巨大datasetの処理速度も表していません。

Nagiはnative整数・連続配列を使い、動的な数値boxing・型判定をループ中に置きません。generated Rustと手書きRustの整数sumの逆アセンブルには、どちらもpacked i64加算の `paddq` が4箇所あります（integer-sum.asm / rust-integer-sum.asm）。LLVMのvector化・unrollがns/itemを下げています。手書きRustとの時間差には生成コード・配置・実行時の揺れがあり、独自backendがRustを上回ったという結果ではありません。

## JSON・view・文字列のコスト

以下のJSONは `id=42,name=alice,age=18` の小さい同一入力です。ns/opはparse/encode等の操作全体、allocationは1回の操作の動的測定です。

| 操作 | ns/op | alloc回数 | realloc回数 | 要求byte累計 |
| --- | --- | --- | --- | --- |
| json_parse_tree | 281.35 | 5 | 0 | 646 |
| json_tree_to_class | 325.72 | 5 | 0 | 646 |
| json_direct_class | 142.31 | 1 | 0 | 5 |
| json_borrowed_class | 178.68 | 0 | 0 | 0 |
| json_class_encode | 113.07 | 1 | 0 | 128 |
| json_encode_reused_buffer | 67.14 | 0 | 0 | 0 |
| json_parse_modify_encode | 289.75 | 2 | 0 | 133 |
| string_parse_i64_1000 | 10486.23 | 0 | 0 | 0 |
| view_1mib | 7.74 | 0 | 0 | 0 |
| copy_1mib | 57511.23 | 1 | 0 | 1048575 |

中間Value treeからUserへ変換する経路に対し、直接Userへdecodeする経路は今回 2.29×、allocationは5→1回、646→5 byteでした。Rust structへ直接deserializeしており、dict/map/汎用JSON treeを経由しません。残る1回は所有Stringフィールドです。

borrowed classは入力を指す&strを使う手書きRustの実験です。0 allocationでも、この試行ではowned classより速くはありませんでした。escapeを含むJSON文字列や入力寿命を越える保持には所有化が必要です。Highのclassにborrowed fieldを許す機能は未実装です。

encode時には128 byte capacityのVecを一つ作ります。reused buffer実験は0 allocationですが、HTTPへ一般適用していません。HTTP responseはnative class → Vec<u8> → Bodyであり、直接socketへserializeする完全streamingではありません。parse-modify-encodeはStringと出力Vecの2 allocationです。

view試験は1 MiB bufferの範囲参照を作るだけです。入力pointer+1とview pointerの一致、0 allocationを記録しました。copy試験は1,048,575 byteを実際に所有コピーします。処理内容が違うため両者の時間比を同じ処理の高速化率とは扱いません。HTTP受信bodyの借用が追加コピーを避けても、network/kernel/HTTP入力buffer全体がzero-copyという意味にはなりません。

## SQLite → class：変換と所有化

同じin-memory SQLiteへ10,000件を事前投入し、cached SELECTから1/100/1,000/10,000件を取得します。id i64、name TEXT「alice」、age i32です。SQL実行・row stepping・class生成・Vecへの格納を含み、接続生成・seed insertは含みません。

| row数 | 列名毎row µs | 列番号 µs | 列番号+reserve µs | idのみscan µs | 列番号alloc | 列番号byte累計 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 0.790 | 0.767 | 0.853 | 0.598 | 3 | 229 |
| 100 | 30.197 | 23.004 | 22.772 | 10.039 | 107 | 10644 |
| 1000 | 327.006 | 184.723 | 209.405 | 94.325 | 1010 | 86824 |
| 10000 | 3398.379 | 2197.449 | 2384.866 | 980.799 | 10014 | 1360624 |

列名をquery開始時に一度だけ番号へ解決する経路は、10,000件で列名を毎row解決する経路に対し 1.55×でした。`FromRow`は直接native fieldを読み、tuple/dict/ORM中間objectを作りません。TEXTはSQLite stepの寿命を越えて返すため、各rowに所有Stringが必要です。row変換全体が0 allocationという主張はできません。

id-only scanとの時間差にはTEXT取得・String allocation・3field変換・Vec格納が混ざり、純粋な変換時間を厳密に分離した値ではありません。reserveは要求byteとreallocを減らしますが、この10,000件試行では通常Vec成長より時間が増えました。全queryで件数を事前に知るためのCOUNTや推測reserveは導入していません。

1,000 insertを一つのtransactionで実行してrollbackする操作は 488.667 µs/op、1 insert+rollbackは 3.463 µs/opでした。Rust allocation counterが0でも、SQLite C allocator、ページ処理、disk I/Oが0という意味ではありません。今回はmemory DBで、永続化/fsync性能は測っていません。

HTTPからのDB jobは専用thread、bounded channel 64、oneshot replyを通ります。その往復・job boxing・async schedulingはこの同期microbenchには含みません。後述のHTTP db_single_rowは実際にその経路を含みます。型付きcolumn検査はruntimeで、コンパイル時schema/SQL検証はありません。

## 実測後の変更

列番号を毎query Vecで保持していた実装を、16列以内はinline配列へ変更しました。列数overflow時のみVecへ退避します。列順入れ替え・missing columnの既存テストを通し、同じベンチを再測定しました。

| 操作 | 変更前 ns/op | 変更後 ns/op | 前 alloc | 後 alloc | 前 byte | 後 byte |
| --- | --- | --- | --- | --- | --- | --- |
| db_indexed_1 | 819.70 | 767.44 | 4 | 3 | 261 | 229 |
| db_indexed_1000 | 213607.44 | 184723.43 | 1011 | 1010 | 86856 | 86824 |
| db_indexed_10000 | 2219904.20 | 2197449.04 | 10015 | 10014 | 1360656 | 1360624 |

確実な効果はqueryごとの1 allocation / 32 byte削減です。10,000件の時間差は小さく、速度改善が統計的に確立したとは扱いません。初期ログはmicro-before.jsonl、変更後はmicro-after.jsonlです。

actorには最大32件のpipeline送信を追加しました。個々のmessageは同じmailboxを通り、全oneshot replyを受けて結果20,000を確認します。一つの「+20,000」にまとめた測定ではありません。結果は下に示します。JSON buffer再利用とDB Vec reserveは実験のみで、現在のHTTP/DB標準経路には入れていません。

## HTTP：同じ成功payloadでの比較

全serverをCPU 0、1 executor/event-loop workerへ固定し、wrkはCPU 7/8、2 thread、64 keep-alive接続です。各case 1秒warmup→3秒測定、各framework/caseを3試行しました。framework順はseed固定で試行ごとに並べ替えます。Nagi/Axumは共通runtimeのtimeout/body-limit middlewareを使います。NagiのみSQLite threadが一つ常駐します。Node/aiohttpにこのmiddlewareを完全に移植したわけではなく、framework全体の同一内部処理を保証する比較ではありません。

| case | workload |
| --- | --- |
| plaintext | GET /health → ok |
| small_json | GET /small → User JSONを毎回生成 |
| post_small | POST /echo、5文字name+age、型とunknown-field検査、同じJSONを生成 |
| post_medium | 同じPOST、4,096文字name |
| query_parameter | Nagiのみ：typed Query limit/name → User JSON |
| db_single_row | Nagiのみ：typed Path id → DB worker → cached SELECT → User → JSON |

Nagi/Axumは同じnative structを使用します。NodeはJSON.parse→field検査→JSON.stringify、aiohttpはjson.loads→slots dataclass→型検査→json.dumpsです。成功status/bodyをそろえています。Nagiだけschema検証を省く測定にはしていません。small_jsonは定数JSON byteの再送ではありません。

rpsとp50/p95/p99は3試行それぞれの値のmedian、maxは3試行の最大値です。複数試行のhistogramをmergeしたpercentileではありません。latencyの単位はµs。errorsはsocket/status/timeoutの合計です。

| framework | case | req/s | p50 | p95 | p99 | max | errors |
| --- | --- | --- | --- | --- | --- | --- | --- |
| nagi | plaintext | 82,701 | 662 | 2318 | 4344 | 14501 | 0 |
| nagi | small_json | 74,344 | 754 | 2355 | 4321 | 109324 | 0 |
| nagi | post_small | 39,860 | 1154 | 9128 | 34048 | 121675 | 0 |
| nagi | post_medium | 36,641 | 1527 | 4072 | 6883 | 89114 | 0 |
| nagi | query_parameter | 68,076 | 814 | 2680 | 4930 | 25215 | 0 |
| nagi | db_single_row | 42,544 | 1410 | 3487 | 6039 | 181247 | 0 |
| axum | plaintext | 87,040 | 632 | 2557 | 6019 | 125302 | 0 |
| axum | small_json | 77,406 | 709 | 2336 | 4224 | 124887 | 0 |
| axum | post_small | 60,842 | 917 | 2568 | 5211 | 16715 | 0 |
| axum | post_medium | 36,366 | 1535 | 3979 | 6783 | 85825 | 0 |
| node | plaintext | 28,606 | 1725 | 6893 | 17529 | 110900 | 0 |
| node | small_json | 35,402 | 1618 | 3741 | 6269 | 25919 | 0 |
| node | post_small | 25,713 | 2203 | 5335 | 10869 | 79994 | 0 |
| node | post_medium | 17,537 | 3275 | 6992 | 15715 | 139972 | 0 |
| aiohttp | plaintext | 17,076 | 3442 | 7607 | 13375 | 90396 | 0 |
| aiohttp | small_json | 14,098 | 3999 | 8526 | 13581 | 47104 | 0 |
| aiohttp | post_small | 11,137 | 5154 | 10084 | 14567 | 27593 | 0 |
| aiohttp | post_medium | 6,518 | 8163 | 21121 | 40894 | 108190 | 0 |

closed-loop wrkの値であり、一定到着率のopen-loopや過負荷時のtail latencyを評価したものではありません。同じruntimeを使う手書きAxumが主要なnative基準です。生成wrapperやhandlerの違い、body処理、allocation、SQLite workerを含む構成差を見て、CPU kernelの差とHTTPの差を分けて評価します。

小さいPOSTはNagi約39.9k req/s、手書きAxum約60.8k req/sで、今回Nagiが低い結果でした。大きいPOSTは約36k req/sで同じ桁です。小POSTの差を特定の関数のコストへ結び付けるprofileは取得できていません。生成wrapperとrouter/stateの扱いを分離する追加測定が必要で、native化だけでHTTPの全経路が最適になるとは判断しません。

次は測定区間のserver CPU使用率、終了時RSSの3試行最大、context switch/s（全server threadの合計差）、thread/FDの最大です。CPU使用率は1論理CPUを100%として、/procのCPU tick差をwall timeで割っています。load generator側のCPU/RSSは含めていません。

| framework | case | CPU % | 終了RSS MiB | switch/s | threads | FD |
| --- | --- | --- | --- | --- | --- | --- |
| nagi | plaintext | 97.8 | 6.35 | 95 | 3 | 10 |
| nagi | small_json | 99.0 | 6.45 | 83 | 3 | 10 |
| nagi | post_small | 98.0 | 6.49 | 66 | 3 | 10 |
| nagi | post_medium | 98.0 | 6.52 | 37 | 3 | 36 |
| nagi | query_parameter | 97.9 | 6.52 | 73 | 3 | 50 |
| nagi | db_single_row | 98.6 | 6.67 | 65,710 | 3 | 10 |
| axum | plaintext | 98.2 | 4.28 | 100 | 2 | 10 |
| axum | small_json | 97.3 | 4.35 | 87 | 2 | 31 |
| axum | post_small | 98.4 | 4.36 | 60 | 2 | 13 |
| axum | post_medium | 99.1 | 4.38 | 32 | 2 | 10 |
| node | plaintext | 99.9 | 51.29 | 856 | 7 | 82 |
| node | small_json | 99.4 | 55.79 | 817 | 7 | 65 |
| node | post_small | 99.6 | 58.84 | 1,072 | 7 | 85 |
| node | post_medium | 99.5 | 68.02 | 1,659 | 7 | 86 |
| aiohttp | plaintext | 99.9 | 28.58 | 14 | 1 | 71 |
| aiohttp | small_json | 99.8 | 28.61 | 13 | 1 | 71 |
| aiohttp | post_small | 99.9 | 28.91 | 13 | 1 | 71 |
| aiohttp | post_medium | 99.0 | 29.83 | 11 | 1 | 71 |

合計54試行でsocket/status/timeout errorは0件でした。正常応答の負荷比較であり、認証、TLS、HTTP/2、slow-client/DoS耐性、大容量streamを含む本番SLO試験ではありません。

## 120秒の継続負荷

Nagi /echo、4,096文字name、128接続、wrk 2 thread、server 1 workerで120秒実行しました。35,238 req/s、p50 3222 µs、p95 7209 µs、p99 16531 µs、max 176174 µs、socket/status/timeout error 0件です。

1秒ごとのRSSは 6.05–8.06 MiB、最終負荷sample 8.06 MiB、停止後5sampleは 7.16–7.16 MiBでした。FDは負荷中 10–138、停止後 10–10でした。

この有限時間で異常や未回収FDが観測されたかを確認する試験です。メモリリーク不在・数日安定性・allocator fragmentation・GC pauseを証明するものではありません。RSSが高水位から下がらない場合も、所有値のdrop、allocatorの保持、fragmentationを分ける追加観測が必要です。全processのallocation/free追跡やheap profilerは未実施です。

## 同時task・TCP接続

task試験はTokioの4 workerです。1,000/10,000/50,000/100,000 taskを生成し、各taskが最初のpollへ到達したことをatomic counterで確認します。Semaphoreを解放するまでは1件も完了できません。その時点のRSSを測り、全taskを解放・joinします。3試行のmedian時間と最大RSSです。gate、Arc、ready counter、spawn/joinのコストを含むテストharnessで、実アプリの1task単独コストではありません。

| 同時待機task | 生成→全join ms | 全待機時RSS MiB | 終了RSS MiB | 未完了 |
| --- | --- | --- | --- | --- |
| 1,000 | 4.812 | 2.34 | 2.34 | 0 |
| 10,000 | 18.389 | 5.62 | 5.62 | 0 |
| 50,000 | 93.424 | 20.28 | 20.28 | 0 |
| 100,000 | 162.528 | 38.64 | 38.64 | 0 |

以前のconcurrency-before.jsonlは「spawnした総数」の試験で、全数が同時に待機していた保証とpeak観測がありません。gateを追加した後の時間とは直接比較しません。完了後のRSSはallocator保持を含み、未完了taskの数ではありません。CPU処理は別のspawn_blocking＋semaphore 4へ分けていますが、開始済みのblocking仕事の強制中断は未実装です。

TCPは実際に1,000/10,000接続を同時保持し、それぞれからhealth応答を受けます。open時のclient同時進行は128、server workerは1です。50,000/100,000 TCPは今回のFD上限16,384とIPv4 ephemeral port 32768–60999の条件で実施していません。100,000 taskの結果を100,000 TCP接続へ読み替えません。

| 要求TCP | 保持TCP | open+応答 秒 | 保持RSS MiB | 切断後RSS MiB | 開始FD | 保持FD | 切断後FD | errors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1000 | 1000 | 0.387 | 28.35 | 28.35 | 10 | 1010 | 10 | 0 |
| 10000 | 10000 | 4.484 | 249.58 | 249.59 | 10 | 10010 | 10 | 0 |

FDは両試行で開始値へ戻りました。RSSは切断直後も高水位を保持しています。約250 MiBで10,000接続を保持できた観測はありますが、約25 KiB/接続の差分はHTTP buffer・runtime・allocator等を含むprocess全体の目安です。kernel socket memoryはRSSに含まれません。

## actor・Supervisor・queue

同じCounterActor、mailbox 64、Tokio 4 worker、20,000 messageを5試行します。serial RPCは1件のreplyを待って次を送り、pipelineは最大32件を進行させます。下のµs/messageはelapsed/messagesのmedian相当で、個々のmessageのp50/p99 latencyではありません。

| 経路 | message/s | amortized µs/message | mailbox | 最大送信batch |
| --- | --- | --- | --- | --- |
| actor_rpc | 22,204 | 45.038 | 64 | 1 |
| actor_pipelined | 554,022 | 1.805 | 64 | 32 |

pipelineはserialに対しこの試行で 24.95×のthroughputでした。待機とscheduler往復を重ねて隠す改善で、1件のRPC latencyが同じ倍率で下がったという意味ではありません。mailbox message/replyのallocationと所有化は残ります。Highのsharedは明示Arcですが、task/channel/DB stateの内部Arcまで消す設計ではありません。

Supervisorはworker panicを3回検出し3回再起動、再起動latencyは 1.951–2.137 msでした。latencyはpanic検出から新しい子taskの最初のpollまでで、1 ms backoffを含みます。独立workerは12回継続しました。crash-loop試験は1秒windowで上限2に達した後、再起動停止を確認しました。SIGSEGV/abort/process killの隔離、request replay/loss、任意Supervisor treeは試験対象外です。

queueは1,000 job、8 worker、channel 64、最大3 attempt、retry backoff 1/2 msで、完了909、dead-letter 91、retry 312、peak in-flight 8でした。失敗はidから決めて注入します。DLQ件数を数える実験であり、永続DLQ、process再起動後のreplay、exactly-once APIはありません。

DB workerを100回生成・終了した後、live worker数は0→0でした。scope子taskのerror/panicでは他の子taskをcancelしてjoinし、guardが解放されたことをunit testで確認しています。外側futureのdropやbody panicではJoinSet Dropによるabort要求までで、その場で非同期cleanup完了を待つ保証はありません。

## 安全性・正しさの確認

| 検証 | 結果と範囲 |
| --- | --- |
| Rust unit test | runtime 23 + compiler 39 = 62件成功。型/range/move/view escape、Low roundtrip、置換signature、実SQLite、scope、actor、Supervisor、queue |
| sample build | 11 High + 1 standalone Low、12件成功。Low call/replaceは42、手書きLowは再生成後も同じbytes |
| real HTTP | 29項目成功。CRUD、型/unknown field/malformed JSON/UTF-8/範囲、body limit、SQL bound param、keep-alive、stream、WS text/binary、timeout後の生存 |
| seeded mutation | compiler parser/check 10,000 + malformed JSON 10,000、seed 305419896、panic 0。coverage-guided fuzzではない |
| code checks | cargo fmt、clippy all-targets -D warnings、locked build/test成功。final-checks.txt |
| 配布検証 | zipを別directoryへ展開し、空のcompiler/native targetからsource build。High valuesは10/16、Low standaloneは4。relative runtime pathを確認。distribution-checks.txt |
| negative ownership | owned/localのview escape、move後使用、借用中move・mutation、taskへのview escapeを拒否。部分move/複雑なflowは最終Rust checkerに依存 |
| pointer/zero-copy | slice pointerのoffsetと0 allocationを確認。安全sliceとbackend borrow checkを使用 |
| resource/fault | 100k同時待機taskの全join、10k TCPの全応答とFD復帰、worker再起動・crash intensity、DB worker終了を確認 |
| 未実施 | ASan/TSan/Miri/Valgrind、coverage-guided fuzz、hardware perf counters、flamegraph、cache/branch miss、全allocator fragmentation、数日soak |

High checkerの成功だけでsoundnessを保証していません。safe Rustを生成し、Rustのborrow/Send検査まで通ったものだけnative binaryにします。runtime側unsafeはSystem GlobalAlloc転送等の限られた観測実装にあり、Lowに任意unsafe/FFIを許す段階ではありません。生成コードとruntimeのdependencyに対する独立したsecurity auditはしていません。

## 実装状況と判断

| 分野 | 今回動くもの | 今後必要なもの |
| --- | --- | --- |
| 二層compiler | indentation High、text Low、各parse/check、native call、signatureを保つ関数置換、Rust native build | crate分割、module/import、source span、汎用generic/trait/function型、self-host |
| native値 | primitive、Copy class、Vec連続格納、nullable、Result、UUID/timestamp | 完全なMap/owned API、method/interface、算術overflow仕様の統一 |
| memory | move、保守的lexical borrow、view、明示copy/shared、Rust drop | High request arena、borrowed class、精密なescape/partial move解析 |
| async/concurrency | Tokio task、scope、bounded channel actor、restart/intensity、retry/DLQ実験 | 汎用actor/queue/Supervisor構文、独自scheduler、durable delivery、cancel可能なCPU job |
| HTTP/JSON | HTTP/1、typed route/query/body、typed response、keep-alive、limit、timeout、WS/stream sample | TLS/auth、HTTP/2、汎用routing、streaming JSON、buffer pool |
| DB | SQLite実CRUD、専用worker、prepared cache、indexed FromRow、typed native fields | 汎用params/pool/transaction、compile-time SQL/schema、PostgreSQL binary protocol |
| Low native制御 | 同じ値型/関数/viewを手書き・生成の両方で利用 | pointer/layout/alignment/alloc/free/unsafe/C ABI/SIMD命令 |

有効だった方向は、native primitive＋連続配列、直接typed JSON、DB列番号の一回解決、不要な小Vecの除去、actor pipelineです。JSON借用やreserveはallocation削減と速度向上を分けて評価すべき結果でした。HTTPはhandler/runtime/serialization/schedulingを含むため、CPU kernelの倍率から予測できません。

次の実装は、算術・borrow仕様の確定、module/generic、任意state actorのlowering、typed DB paramの一般化、request arenaとbuffer再利用の実測の順が妥当です。完成に必要な工数を今回の試作速度だけから推定しません。Low self-hostはString/Map/module/allocator APIとbootstrap一致試験をそろえた後の段階です。

## 再実行と生ログ

ビルド・機能試験はREADMEの手順を使用します。数値の再測定時は同時にビルドや別のCPU benchmarkを走らせず、CPU affinityを環境に合わせて変更してください。

```bash
taskset -c 0 ./native-target/release/nagi-cpu > benchmarks/results/cpu-nagi.jsonl
taskset -c 0 ./target/release/examples/microbench > benchmarks/results/micro-after.jsonl
taskset -c 0 python3 benchmarks/python_cpu.py > benchmarks/results/cpu-python.jsonl
taskset -c 0 node benchmarks/node_cpu.js > benchmarks/results/cpu-node.jsonl
./target/release/examples/concurrency_bench > benchmarks/results/concurrency-after.jsonl 2> benchmarks/results/fault-after.log
python3 tests/connections.py > benchmarks/results/connections.json
python3 scripts/http_bench.py --wrk /absolute/path/to/wrk --soak 120
python3 scripts/summarize_results.py
```

results/にはCPU・JSON・SQLite・allocation・concurrencyのJSONL、wrk各試行のtext、HTTP集約JSON、1秒resource sample、障害stderr、環境情報、checksum検証を同梱します。wrkのソース全体は同梱せず、commitとLua scriptを記録しています。Linux専用のtaskset/proc計測部分は他OSでは置き換えが必要です。CI定義は同梱していますが、remote CI上では今回実行していません。

設計・未実装の詳細はdocs/の22ページ、再現用ソースはcompiler/runtime/examples/tests/benchmarks/scripts/にあります。
