# Nagi 0.2.2 正式公開準備

2026-10-27。0.2.1以降にmainへ統合した互換性維持のbuild・依存・入力・教材改善を0.2.2候補として固定する。workspace版と稼働するlock内のlocal package版、正式CHANGELOGを揃える。JetBrains・VS Codeの版、既定release/full、High/Low意味論、安全検査をこの版更新の都合で変えない。

Result/`try`教材PR #176はmainへ統合済みで、この候補はその統合後のmainをbaseに最終検証する。BrowserLoginの期限診断PR #169は原因修正ではなく、別Draftのため候補へ含めない。既にmainへ入った限定診断は、間欠504を修正済みとする根拠に使わない。

## 対象と保証

- `--runtime-profile auto`は、検査済みIRが対応する小さな同期programだけ共有sourceのcore packageを選ぶ明示opt-in。既定はfull。unknown、async、adapter、追加Rust依存、incrementalは理由付きでfullへ戻り、Cargo失敗をfull再試行で隠さない。
- `--seed-lock`は初回Cargo解決の明示入力。既存lockを常に優先し、破損lockを置換しない。整合性・regular file・上限・入力identityを検査し、Cargoを毎回実行する。記録したcold fullでは遅いため速度機能として既定化しない。
- `read_line(prompt)`は従来の`read_line()`を維持する。promptはborrowし、出力、flush、入力の順で各I/O失敗を`Result`へ返す。`try`のErr伝播やHigh/Low互換性を変更しない。
- rustls/WebPKIの修正版依存を使用する。インストール済みruntime、生成アプリ独自のlock、既存exeは自動更新されない。新配布runtimeで再解決・再ビルドする。
- 利用者向けCI generator、runtime/seedのcache key、世代管理、receipt、旧成功exe、失敗時の旧latestを維持する。最適化の観測からnative成果物を独自復元しない。

実装済みの範囲、fallback、測定条件は[runtime選択](runtime-profile-selection.md)、[seed lock](lock-seed-selection.md)、[LTS候補検証](lts-candidate-validation.md)と各benchmark記録を参照する。compiler query cache、test結果cache、全asyncの分割、自動crate粒度最適化、PostgreSQL、WebSocket、HTTP/2、streaming、server-side TLSは0.2.2の完成機能ではない。

## 最終gate

候補HEADでrelease plannerがNagiだけを0.2.2として選び、VS Code・JetBrainsを版更新扱いにしないことを確認する。Linux x86_64、Windows x86_64、macOS ARM64、macOS Intelの4配布、抽出後のversion/help、外部projectのHigh・保存Low・native、installer/update/uninstall、seed/full/core、checksum、旧成功成果物を省略しない。通常3OS L2や過去HEADのL3で候補HEADを代用しない。

PRのversion更新と正式CHANGELOGは検証用package/L3を起動するが、pull request eventでは公開しない。main pushではReady・L3・release planが揃うと`publish-release`がtagとGitHub Releaseを作り得るため、このbranchのmergeは正式公開の明示許可を得るまで行わない。公開後は`nagi-v0.2.2`のcommit、draft/prerelease、4 archive/checksum、同梱runtime・seed・tool、installerのLatest選択を読み戻す。

## 残る判断

- BrowserLoginのvalid requestがL3で504になった事象は、保存した限定診断と後続成功があるが根因未確定。再実行成功を修正証明にしない。0.2.2公開を止めるseverityか、既知の間欠CIとして追跡を続けるかを最終候補の再現状況と独立reviewで判断する。
- LTS指定とサポート期間は別の未採用案。0.2.2公開だけでLTSを約束しない。
- JetBrains 0.1.6とVS Code 0.1.13は別component。compiler metadataの改善を理由にpluginを同時公開しない。
