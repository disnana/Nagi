# LTS候補の検証記録（2026-10-25）

[English](lts-candidate-validation.en.md) · [運用方針の提案](lts-candidate.md)

対象は[#160](https://github.com/disnana/Nagi/pull/160)を統合したmain `22183d7233950bdb575b62a72393dac9b4aa0767`、tree `a7a2e3985847a1bbd2fca8be22b05f5c69018322`。PR最終treeと一致し、検証したcompiler SHA256は`49aa30fbf2b0eec6067755f1c1b72408e9a0d650d6862f2652b3fd4ecf887f9c`。この記録の追加は実装を変更しない。0.2.2の版更新・LTS指定・公開は未実施。

## L3と公開の分離

[既存の検証レベル](ci-levels.md)に従い、`gh workflow run ci.yml --repo disnana/Nagi --ref main -f level=release`を実行した。手動実行はpublish条件の`event=push`を満たさず、版・CHANGELOGも変更していない。

- [L3 run 40152653289](https://github.com/disnana/Nagi/actions/runs/40152653289)：対象SHAは上記main。4 OS/MSRV・browser/TLS・install/upgrade/uninstall・配布物・Readyを既存手順で検証するrun。完了結果はこのrunと対応PRの検証欄で確認する。開始要求の成功を検査成功とは数えない。
- [先行main L2 run 40152458417](https://github.com/disnana/Nagi/actions/runs/40152458417)は成功した別run。#160の3 OS成功もL3のmacOS Intel検証へ転用しない。

## 実用例での選択と互換性

Linux x86_64、Rust/Cargo 1.99.1、dev/debug0/jobs2/offline、専有の共用target。既存の`verify_application_examples.py`のcheck/build/既存smoke assertionとreceipt検証を再利用した。High7例は独立保存Lowでも検査・build・実行し、手書きLow1例を加え、各full/autoで30構成が成功した。full/autoとHigh/Lowのsmoke結果は一致した。全build statsで`checks=full`・`cargo_always_invoked=true`を確認した。

| 既存例 | 構成数 | auto選択 / 理由 | 1構成の既存smoke件数 |
| --- | ---: | --- | ---: |
| Hello/Point | 4 | core / eligible_core | 1 |
| stock-report | 4 | full / unsupported_ir | 12 |
| device-settings | 4 | full / unsupported_ir | 12 |
| seat-reservations | 4 | full / unsupported_ir | 1 |
| supervised-worker | 4 | full / rust_adapter | 12 |
| file-json | 4 | full / rust_adapter | 4 |
| auth-boundary | 4 | full / rust_adapter | 52 |
| order-quote（手書きLow） | 2 | full / unsupported_ir | 23 |

Unicode・10,000行JSON集計、SQLite NULL/BLOBと再open、actorの業務Err/restart、task panic後のcleanup、ファイルadapter、認可の正負入力を既存の期待値で確認した。case数は各smokeが返す単位で、全テスト数や保証の強さを比較する尺度ではない。HTTP/DB/認証/asyncのcore対応は追加していない。未使用の未対応関数もfullを選ぶ。一般アプリの高速化としてPointの結果を一般化しない。

対応・fallback・cache無効化の契約は[日英runtime profile説明](../runtime-profiles.md)を参照する。型・所有権・Policy検査を省略せず、Cargo失敗をfull再試行で隠さない。compiler17入力とアプリ入力hashは実行前後で不変。専有target最大711.2MiBを終了後に削除し、保持した30 exeのSHAと30 receiptを再確認した。

## 速度の別々の観測

今回の互換性再生の累計はcheck **0.261秒**、build **73.837秒**、smoke **17.523秒**。buildは同じtargetを順に暖め、full/autoの順序を均衡化していないため速度倍率を算出しない。smokeにはHTTP待機や否定ケース等を含み、プログラム自体の実行速度とは異なる。

同じseat-reservations nativeシナリオだけをbuild後に各3回実行すると、fullは中央値14.689ms（14.513–14.822）、autoは14.776ms（14.570–14.963）。両方の実選択はfullで、起動時間と自己検証を含む。範囲が重なり、小標本のため優劣・定常throughputを主張しない。

full/coreの均衡化した既存比較と既定releaseのCPU probeは[別の測定記録](../../benchmarks/results/runtime-profiles-2026-10-24/README.md)にある。小さい対応例のdev build、既定fullのrelease実行、今回のfallback互換性を別々の証拠として扱う。CI全体の短縮率や費用削減は未計測。

VS Codeの既存`npm test`はNode 24.19.0、候補compilerをPATHへ明示して3回実行し、各196成功/失敗0/skip0（22ファイル）。wallは1.286/1.130/1.171秒、中央値1.171秒、範囲1.130–1.286秒。初回compiler欠落のENOENTと、通常sandboxでのsnippet起動EPERMを保存し、同じテストを適切な実行許可で再実行した。テスト・assert・timeoutを変更していない。これはNode/compiler test時間であり、IDE GUIの応答時間ではない。

[機械可読の測定・対応表](../../benchmarks/results/lts-candidate-2026-10-25/summary.json)に実行・build・smokeを分けて記録する。今回の全workspaceテスト時間のbefore/after比較は行っていない。先行#160の全Rust2,029成功と最新L3で正しさを検証し、削除・skipによる「高速化」は行わない。

## 分かりやすさと導入負担

公開compilerの導入は[既存手順](../getting-started.md)、アプリの通常ループは`nagic check --project .`→`nagic run --project . --profile dev`。native実行にはRust/Cargo・platform build tools・runtimeが必要で、初回の依存取得/compileは残る。autoは候補compilerのhelpを確認して明示し、公開0.2.1へ渡さない。

現在の例の物理行数（空行・コメントを含む、smoke除外）はstock-report Nagi71/Rust0、device-settings132/0、seat-reservations142/0、supervised-worker210/39、file-json22/16、auth-boundary188/160。API/認可条件を含む例と単純なHelloを同等に比較しない。Python同等アプリとの条件を揃えた行数比較ではなく、Rust adapterの負担も隠さない。

`x: i32 = "wrong"`の最小例はcheck exit1となり、元2行目、期待i32/実str、明示変換の案内を表示した。これは一つの診断観測であり、全診断に自動修正案がある保証ではない。短い記述を目指して型・所有権・認可の明示を削る提案はしない。

## プラグインの範囲

| 機能 | VS Code | JetBrains |
| --- | --- | --- |
| 補完・説明・定義 | compilerを使うsemantic候補/hover/signature/definition、snippet、High/Low色付け | compiler-backed completion/docs/parameter info、正確なtargetへの移動、High/Low色付け |
| 診断 | 保存時checkと手動check。標準15秒の期限は応答実測値ではない | 背景snapshot解析と明示Check。古い型/診断を新snapshotへ流用しない |
| 実行 | check/lower/build/run taskとtrust確認 | 標準Run/Stop、save-before-run、trust確認、dev既定・明示release。compiler capabilityでflagを確認 |
| テスト | 現時点で専用Nagi test explorerはない。既存アプリharness/CIを利用 | 専用Nagi test runnerはない。Check/Runと外部harnessを利用 |

現ソースではJetBrainsにdev/release選択がある一方、VS Codeの標準taskはCLI既定のprofileを使い、dev専用設定はない。両プラグインに`--runtime-profile auto`専用の選択UIはない。今回のcore選択の計測をIDE Runの既定速度とは扱わず、autoを試す場合は対応compilerのCLIで明示する。専用UIやtest explorerの追加は今後の提案とし、この候補で新機能として約束しない。

今回のNodeテストはVS Code実GUIの受入ではない。JetBrains0.1.6はGitHub公開・利用者受入済みだが、今回の新しいGUI再検証とは区別する。公開記録を再確認し、日英READMEの0.1.6「候補/未公開」という古い表記をこの差分で訂正した。Marketplaceの公開状態をGitHubタグから推測しない。Verifierの互換性成功に伴うdeprecated/experimental API警告は別差分の調査対象であり、この作業で解消していない。

JetBrainsの[既存入力測定](../../editors/jetbrains-nagi/benchmarks/python-editing-2026-10-22/README.md)は旧commit/IDEA2025.1.1のaction計測（warmup3/sample20）で、現在HEADのGUI/IME・描画・end-to-end時間ではない。compiler queryの時間にもIDE debounce/PSI/renderは含まれない。新規GUI/長時間操作の安定性、将来IDE版、未検証compiler組合せは未確認とする。

## セキュリティと残件

既存security/MSRV/browser/TLS回帰とCodeQLの実行結果は対象commitに紐付ける。Tokio lockは1.54.1だが、その一件だけで他の依存を安全としない。依存アラート一覧の取得（`repos/disnana/Nagi/dependabot/alerts`）はintegration権限不足のHTTP403で拒否され、未取得。迂回や同じ権限での再試行はしていない。

別途、公開RustSec DB `21d909e4c8bd4e14c7aea58bf980c881a0053ab1`（更新2026-10-24）とcargo-audit0.22.2で元Cargo.lockを照合した。ignore・OS/arch絞り込み・stale許可なし、exit1でTLS依存の既知version一致6件を検出した。静的triageでは現固定profileの必要条件を欠く4件と、設定された信頼root/証明書経路の確認が必要な2件に分け、Nagiでの悪用成立を確定したものは0件。任意のRust依存によるfeature統合は別graphで再確認が必要。既知の修正版への最小依存更新は[別Draft #162](https://github.com/disnana/Nagi/pull/162)で扱い、更新後の照合・互換性・L3が揃うまでLTS可とはしない。一般的な新規security scanや実攻撃の検証を行ったわけではなく、脆弱性ゼロと断定しない。

修正候補は`55d785a145437be773cc139e57da3fbbdae636ff`。runtime manifestと2 lockだけを更新し、rustls0.23.46/WebPKI0.103.16/roots1.0.10を要求する。既存アプリの継承lockも通常buildで全3版が更新され、旧exe/receiptを保持した。同じDBで両lockの既知版一致6→0、HTTP client221（うちTLS固有15）/公開API1/registry3/contracts11/native2、Clippy、既存Rust1.88 Ed25519 core14/public1、全workspace2,029成功・失敗0・既存ignored4。949.59秒は今回の全体回帰1回の観測で、before/after比較ではない。独立した最終reviewも具体的指摘なし。[修正候補の4 OS L3](https://github.com/disnana/Nagi/actions/runs/40154996082)と[#162](https://github.com/disnana/Nagi/pull/162)で完了状態・結果を確認し、元mainのL3と分けて判定する。文書の更新時刻や過去の状態表示ではなく、対象SHAの実際の完了jobを根拠とする。

さらに公式WebPKI VCS `8ecdcc76f56b68fda67a6adf7976f5eed5006a4c`と公開crate library/data30ファイルの一致を確認し、無編集の既存`distrusted_spki`回帰1件が成功した（119 filtered、全上流testを実行した主張ではない）。同じAWS-LC版/featuresだが上流dev/test全graphは別の検証lockであり、Nagi配置での攻撃再現ではない。上の30構成・速度測定は元mainの記録で、TLS修正後の速度へ転用しない。

前回Windows SF06の原失敗は元log欠落で根因不明。診断保存の改善と後続成功は根因修正の証明ではなく、flakyの可能性を残す。[運用案](lts-candidate.md)の担当・残存リスク判断が必要。SECURITY.mdの日付/公開版/未完了記述にも古い情報があり、適用前レビュー用の差分案を別途準備した。現物の方針は未変更で、LTS指定前に実際のrelease履歴と現契約へ整合する必要がある。本提案から既存security保証を追加・削除しない。

今回のローカル再生はLinux/dev/loopbackと合成credentialのみ。releaseアプリ、他OS、外部TLS/本番構成、長時間負荷、Axum/quote-api/byte-inspector全smoke、配布展開実行は別途L3の証拠と区別する。4 OS CI成功だけでも実アプリの全配置や長期安定性を証明しない。
