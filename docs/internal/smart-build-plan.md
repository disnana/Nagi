# Secure by Designとアプリ開発基盤: 0.2.0調査と第1差分

対象base: `2c703b89645b164f586a926e473d08395be65c90`（公開0.2.0後のmain）。公開releaseを変更・再発行しない。現物のコード、契約、CIを照合した。以下は全repositoryの脆弱性監査完了宣言ではない。

## 現物の構成と制約

`source.rs`がimportを読み込み、module/DefIdを保持。`emit.rs::cli`はHighの名前・型・所有権を検査し、生成Lowを独立parse、`check::finalize`でchecked factsと生成planを封印する。`checked.rs`/`view_flow.rs`/`capabilities.rs`の保証を省略しない。Rustは`emit.rs`から単一binへ生成され、`nagi-runtime`は独立crate。`graph/{modules,calls,types}.rs`は既に関係を抽出するが、function valueやcallbackを完全には追跡しない。

`generation.rs`とADR007には既に、canonical source/outからのapp ID、常設OS lock、毎回新しいbin、immutable成功snapshot、Cargo.lock継承、atomic latest、run前unlock、IDE管理runのlease/GCがある。既定と明示`NAGI_NATIVE_TARGET_DIR`の共有依存cache、入力file identity保護も実装済み。mtime-onlyなビルド基盤を新設する課題ではない。

現在のCLIは常にreleaseを要求する。新しいgeneration binは孤児Cargoによる上書きを防ぐ一方、アプリbinのCargo targetが毎回変わる。Nagiが関数差分を検出しても、そのまま関数ごとのRust成果物を再利用できるわけではない。現状のDefIdと生成symbolにはcanonical module pathが含まれ、checkoutの場所が違えば同じ内容でも変わる。移植可能なcacheには、論理module identityと診断用物理pathを分離する追加設計も必要。genericのmonomorphization、crate依存、profile、build script、include、target、toolchain、Cargo設定・環境はRust/Cargoの領域である。Nagiソースだけのキーでnativeを再利用しない。

## 安全性の対応表

| 対象 | 実装済み | 不完全・未対応／アプリの責任 | 根拠 |
|---|---|---|---|
| HTTP server | loopback HTTP/1.1、明示Policy、request lease、authority、本文/header/応答/接続/期限/rate予算、panic境界、shutdown | 標準inbound TLS・HTTP/2・upgrade・任意streaming・peer/proxy信頼APIなし。公開時のTLS終端・信頼proxy設定は配備責任 | `runtime/src/http_server.rs`とsubmodules、`docs/http-server.md` |
| outbound HTTPS | 明示operation/endpoint、証明書・名前検証、接続先IP検査と接続の束縛、上限と期限、redirect/retryなし | 任意URLの便利factoryなし。許可先・DNS・egressとtrusted Rustの監査をアプリが担う | `runtime/src/http_client/`、`docs/http-client.md` |
| 認証・認可 | request-bound AuthScope/Grant、明示public/authenticated/authorized Policy、proof非偽造・SameTask、失効、Ed25519検証入口 | business verifier/authorizerの正しさ、public routeの選択、外部効果の対象結合はtrusted adapter境界 | `auth.rs`、`auth/ed25519`、`check/security.rs`、`docs/security.md` |
| session/CSRF/CORS | SQLite session、cookie属性、unsafe method guard、Origin、CSRF、exact CORS/preflight、checkpoint、login境界 | store同一性など動的検査。GET等を無副作用に保つのはアプリ責任。任意Rust内の迂回は証明しない | `auth/{session,csrf,prelogin}`、`http_server/{origin,cors,browser_login}` |
| XSS | typed HTML subset、text/属性のescape、管理されたURL/応答finalizer | 任意HTML/JS/CSSのサニタイザーではない。trusted Rustのraw出力は別境界 | `runtime/src/html`、`stdlib/html.rs`、`docs/security.md` |
| SQL | SQLite literal Query、typed Parameters、専有Tx、容量とclose/cleanup、opt-in offline schema prepare | PostgreSQL未実装。deploy schema一致、実行時NULL/型、tenant predicate、受理済み効果のrollbackは別。文字列連結SQLへ緩めない | `runtime/src/sqlite/`、`check/sqlite.rs`、`sql_check/` |
| async/所有権 | Tokio、structured Scope、Task一回consume、actor/supervisor予算・終了、Rust Drop/Future | non-yielding処理の強制停止、OOM/abort回復、既に受理したDB/I/O効果の巻戻しなし。最終Send/Syncや任意Rustはnative検査 | `concurrent.rs`、`task.rs`、`actor/`、`view_flow.rs` |
| WebSocket | Axumにws featureとtokio-tungstenite依存がある | Nagi標準upgrade/APIなし。旧 `/ws` は404。依存が存在することを標準対応と数えない | `runtime/Cargo.toml`、`tests/http_integration.py`、`docs/http-legacy.md` |
| QUIC/WebTransport/WebRTC | 標準APIなし | trusted Rust adapterは可能だが共通Policy、lease、予算、取消・shutdownの新しい契約が必要 | 標準registryとtransport契約 |

### 追加transportの順位とRust候補

2026-10-20にcrates.io APIで候補の存在・公開版・repositoryを確認した[記録](../../benchmarks/results/smart-build-2026-10-20/transport-crates.json)。版の安全性、保守品質、全target互換性を監査した意味ではない。新依存は今回追加していない（compilerのSHA-256はruntimeですでに使用中の同版sha2を直接参照し、依存graphの新crateなし）。

1. **WebSocket**: 既存Axum ws / [tokio-tungstenite](https://crates.io/crates/tokio-tungstenite)を先に評価。`upgrade(request, policy, options)`のようなroute-bound追加APIを候補とし、Origin/認証、接続所有者、frame/message/queue上限、backpressure、heartbeat、各messageでのlease/権限再検査、close deadline、shutdown drainを先に固定する。HTTPの一回限りGrantを長寿命接続で無期限に保持しない。公開署名や既定予算は未確定。
2. **QUIC**: [Quinn](https://crates.io/crates/quinn)はRustls/Tokio系の候補。既存HTTPより先に一般socket APIを公開せず、TLS1.3/ALPN、stream/datagram数・byte予算、0-RTT replay、migration、cancel/close、authorityの契約を決める。認証済みでも0-RTTの状態変更を安全と推定しない。
3. **WebTransport**: [wtransport](https://crates.io/crates/wtransport)はHTTP/3上の候補。QUICだけで対応済みと数えない。CONNECT/Origin、browserの相互運用、sessionとstreamの階層予算、messageごとの認可、datagramのlossと終了規則が必要。
4. **WebRTC**: [webrtc-rs](https://crates.io/crates/webrtc)かSans-I/Oの[str0m](https://crates.io/crates/str0m)を用途別比較。Nagiバックエンドのdata channel用途を先に限定し、signaling認証、SDP/ICE入力制限、STUN/TURNへのegress、IP漏洩、DTLS/SCTP、再接続・取消を設計する。メディア全体の標準化は別計画。

「安全な既定値」は既存の明示予算やPolicyを削除して短縮する意味にしない。一般用途のoptions builder/recipeを別途計測し、適用範囲を説明できる場合に追加する。未知の適正値を今回の標準defaultにしない。

## 採用する小さな差分と確認境界

1. **今回実装**: `--profile dev|release`（既定release維持）。Rust不要の既存checkをCIの先頭に分離。完全検査・世代隔離をそのまま使う。
2. **今回実装**: opt-in `--build-stats` と解析／整合性記録。署名・型／正規Low関数／生成RustのSHA-256、既存module/call/type graph、Cargo fresh件数と時間。receiptはsnapshot/manifest/lock/exe/実compilerのbyteをhashし、公開前に作成失敗なら旧latest維持。照合scriptは固定path集合、schema、app/generation、hash/長さ、linkを検査する。攻撃者がreceiptごと差し替える場合の認証ではない。
3. **今回実装**: 一般アプリ向けGitHub Actions生成。公開0.2.0のCLIに合わせ、新しい未リリースflagを配布版へ渡さない。OS/arch/toolchain/lock/Nagi版でcacheを区別し、PRはrestoreのみ、成功default-branch pushのみsave。全generated snapshotや秘密をcacheしない。
4. **次の差分**: まず小さなアプリでも約200のCargo artifactを作るruntime依存graphを測る。公開0.2.0の全APIを既定で維持したまま、HTTP/TLS/認証/SQLite等の明示opt-in feature構成と、アプリ間の依存target共有を比較する。任意Rust adapterが使うruntime APIをNagi importだけから削らない。温まった小アプリの生成・検査は数msであり、frontend query化より依存cacheとcold最適化を優先する根拠になる。その上で生成コードを本物のCargo libraryとしてCASへ分離し、世代固有binをthin shimにする案を試作。`crate::`参照、adapterのcrate環境、diagnostic mapping、lock/retention、実行中旧exeを保つ。`include!`のnested module化や文字列でmainを書換える案は採らない。
5. **次の差分**: sealed checked factsを再利用可能なqueryへ分け、compiler executable/features/configと全Nagi/Low/標準registry入力をキーへ含める。public signatureとbody changeの伝播を別にする。今のgraphは不完全なので、到達しないという理由で検査・テストを除外しない。未知edge/extern/callbackは保守的に全体へfallback。
6. **その後**: テストの純粋性・外部効果・入力閉包を明示するtest manifest、独立testの並列化、結果cache、merge前の完全/security suite、watchのdebounceと取消・再検査。アプリのuniversal `nagic test` やhot reloadは未実装。無停止更新とprocess再起動を混同しない。

利用者のcheck/build/runを主対象とする。rootの大きなCIを削って速く見せる変更はしない。今回は既存CIへ新しいscriptの検査だけを追加する。全体CI時間と課金はジョブ重複・待機・matrixを含めて別に評価し、ローカルbuild短縮からCI全体の短縮を推測しない。

## 検証と残作業

[計測・検証記録](../../benchmarks/results/smart-build-2026-10-20/README.md)に実測と未測定を分ける。既存generationのlock/barrier、孤児Cargo、alias保護、旧exe、公開manifest、projection/latest失敗の回帰を維持する。新規のprofile/receipt失敗、import本体変更、High/保存Low、native変更、managed dev、整合性不正を追加した。

**残る優先度**: P1・将来の誤cache reuse防止には完全な入力境界と独立監査が必要（現在はNagi独自reuseを有効にしない）。P2・Rust library分離とfrontend query化、アプリtest契約、CIの実ジョブ計測。P2・WebSocketの長寿命権限／終了契約。QUIC/WebTransport/WebRTCとPostgreSQLは未実装で、差分に混ぜない。4 OS実行と生成利用者workflowのGitHub実行はローカルLinux成功と別のacceptanceである。
