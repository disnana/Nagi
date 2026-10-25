# Nagi 0.2.2: 開発体験・発見性・性能の改善

基準は公開済みNagi 0.2.1/main `9fe5e079`。0.2.2は未公開で、version bump・tag・公開は検証後の別工程にする。

**現在状態（2026-10-25）:** 公開記録ではNagi 0.2.1は10月23日、JetBrains 0.1.6は10月24日に公開済み。0.2.2候補は開発中である。#150のcompletion marker修正はmainにmerge済み（merge commit `9a3a975`）であり、以下に残るCI障害の記述は当時の計画snapshotとして読む。JetBrains 0.1.4のGUI確認も古い候補時点の記録で、現在のrelease blockerではない。現在の公開draftや未完了事項は該当するPR/Checksで確認し、この計画の古いsnapshotを上書きせず追記する。

## 小さくレビューする範囲

1. **開発者Docs・AI Skill・公開サイト**: 目的から探す早見表、Nagiの価値とPython/FastAPI・Rust/Axum・Goの開発方式比較、0.2.1基準のAI開発/デバッグ手順、明示した公開資料の`llms.txt`索引。公開版と開発中機能を区別し、比較は実測のない速度倍率を主張しない。
2. **frontend/editorの測定と改善**: 同一toolchain/profileでbaselineと候補を比較し、High/保存Low・overlay・不完全入力・UTF-16・importの応答を照合。通常ケースの悪化が残る候補は採用しない。全検査や型・所有権・Policyを省略せず、Cargo/nativeの速度と混同しない。
3. **安定性とrelease検証**: 既存CIの失敗を原因ごとに直し、必要なOS・IDE・native回帰と独立レビューを通す。**初回計画時点の記録:** #150 completion marker修正はWindows/macOS ARM/Linuxの回帰が成功したが、別のIntelliJ EAP fixture teardown失敗が残り、PR全体は未成功だった。この記録はmerge前のsnapshotで、現在の#150状態は冒頭のcurrent-stateを参照する。後続PRのCIも各headで確認し、再実行のみで完了扱いにしない。

## 最初の実装・検証

Docs/Skill/siteの差分は言語・runtime・既定profileを変更しない。サイト生成と公式/forkのdiscovery回帰2件で日英110ページのリンク・見出し・assetsを検査し、入門/SQLiteの日英snippet整合とCI判定74件を確認した。公開0.2.1 CLIで比較ページの日英例とAI言語例を`check`した。独立したDocsレビューのstandalone Skill参照先不足を修正し、再レビューで指摘なし。これはSkillで任意のアプリを正しく開発できる保証でも、GEOの推薦効果の測定でもない。

parser由来tokenをsymbols/assistへ再利用する最初の候補は、字句負荷が高いcaseでは改善したが、500localsのassistで悪化した。範囲を狭めても15回すべて遅く、中央値128.957→136.639ms（約6%悪化）だったため、本体変更を取り下げた。比較用の測定基盤と不採用理由は別の差分として保存する。速度改善を実装済みとは数えない。

## 0.2.2を公開する前の条件

採用したcodeごとに必要な回帰・native動作・独立監査を実施し、最新commitの必須CIを確認する。docs-onlyの検査結果を処理系全体の保証へ拡張しない。Nagi自身のビルド、利用者アプリのビルド、editor応答、native実行は別々に計測する。query/cacheの誤再利用、旧成功成果物の破壊、High/Lowの意味論変更を許容しない。

性能比較の追加、検索UIの拡充、より広い言語機能は、根拠と検証可能な単位が揃ったものから採用する。未計測の速度や未実装APIを0.2.2の約束に含めない。作業の区切りで専用target等の容量を確認し、再生成可能な一時データだけを証拠確保後に整理する。
