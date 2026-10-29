# Fumadocs 言語切替 UI 作業チェックポイント

対象は公開 `main` の `950a84effe85c6eb4c5e6bd8b0a9bd577a90368e`、専用worktree `/workspace/Nagi-fumadocs-language-control`、branch `fix/fumadocs-language-control-20261029`。

## before の確認

保持済みの公開サイト画像を確認した。

- `/workspace/evidence/nagi-pr200-live/ja-changelog-desktop.png`（1280×2111）では、`English` がサイドバーの教材・変更履歴と同じリンク群に単独表示され、最下部のGitHub・theme controlsから離れている。
- `/workspace/evidence/nagi-pr200-live/ja-writing-mobile-dark.png`（360×1888）はmobile dark表示の保持画像。language UIを含む最新のmobile状態は再撮影・確認が必要。
- 同じ証拠の `live-browser-results.json` は公開サイトの確認結果（2026-10-29 04:28 UTC）を記録しているが、今回の変更後比較には使えない。

基準sourceは `website/components/locale-switch.tsx` の単一locale linkと、`website/lib/layout.tsx` の`baseOptions()`から渡すsecondary custom nav item。現在のtoggleは `usePathname()` から反対localeへ移動する。

## 採用予定の範囲

Fumadocs既存sidebar utility slot/APIを確認したうえで、theme・utility controlsに揃ったcompactなaccessible language menuへ置く。current localeを常時読める表示にし、選択肢は日本語/Englishにする。同ページへのlocale切替でpathnameとanchor hashを保持し、keyboard操作、focus、mobileの表示、light/darkの両方を維持する。周辺レイアウトの全面変更はしない。

## 状態と再開項目

source変更・npm依存導入・検査・GUI確認・after screenshotはまだない。容量確認時点の空きは11 GiBで、8 GiBの下限は満たしていた。`/workspace`内に再利用できる`node_modules`は見つからず、npm cacheは568 KiBだった。依存を導入するときは新worktreeに一度だけ配置し、空き容量を再確認する。

再開時はFumadocs utility slot/APIと既存component patternsを先に確認し、locale menuと同ページ・hashの対応を小さく実装する。その後、unit/build/browser検査、keyboard/focus、mobile、light/darkのGUI確認を行い、before/after screenshotを保存してからdiffを見直してcommitする。

後続作業TODO:

- 言語controlの自然な配置とcurrent language表示を仕上げる。
- home pageでdark modeをOFFにできない問題を別scopeで調査する。
- Nagiらしさが弱い見た目を別scopeで検討する。
- discord.py type referenceの要求は次段階として扱う。この修正へ混ぜない。

このチェックポイント時点では、実装完了や検査済みとは扱わない。push・PR・merge・releaseは行わない。
