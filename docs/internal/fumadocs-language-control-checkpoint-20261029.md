# Fumadocs 言語切替 UI 作業チェックポイント

実装基点は `main` の `950a84effe85c6eb4c5e6bd8b0a9bd577a90368e`。最終確認時はWindows GNU恒久修正後の `main` `bde4e4d447a3268b9d81f9e355df864ec1a5c472` を統合した。

## before の確認

保持済みの公開サイト画像を確認した。

- `/workspace/evidence/nagi-pr200-live/ja-changelog-desktop.png`（1280×2111）では、`English` がサイドバーの教材・変更履歴と同じリンク群に単独表示され、最下部のGitHub・theme controlsから離れている。
- `/workspace/evidence/nagi-pr200-live/ja-writing-mobile-dark.png`（360×1888）はmobile dark表示の保持画像。language UIを含む最新のmobile状態は再撮影・確認が必要。
- 同じ証拠の `live-browser-results.json` は公開サイトの確認結果（2026-10-29 04:28 UTC）を記録しているが、今回の変更後比較には使えない。

基準sourceは `website/components/locale-switch.tsx` の単一locale linkと、`website/lib/layout.tsx` の`baseOptions()`から渡すsecondary custom nav item。現在のtoggleは `usePathname()` から反対localeへ移動する。

## 採用した範囲

Fumadocs 16.17.9の既存language utility slotを使い、theme・utility controlsに揃ったcompactなmenuへ置いた。current localeを常時表示し、日本語/Englishを選べる。同ページへの切替ではpathnameとanchor hashを保持する。既存のcustom secondary linkは削除し、周辺レイアウトの全面変更はしていない。

Fumadocs標準buttonの既定`aria-label`は表示中locale名を含まず、axeの`label-content-name-mismatch`になった。slotのrootだけを公開APIで包み、表示中localeをaccessible nameへ含めた。検査を無効化せず、日英・light/darkで同じ規則を適用した。

## 検証結果

- Fumadocs 16.17.9の公開typeと実装を照合し、`slots.languageSelect`と`onLocaleChange`の契約を確認した。
- `npm run typecheck`、191 static pagesの`npm run build`、content 41 files/locale・18 versions・58 legacy routes、teaching evidence 11件が成功した。
- Chromium browser test 11件が成功した。日英の同一path/hash、focusとEnterによる選択、390px mobileでlanguage/themeが同じutility rowにあること、theme、検索、copy、legacy alias、changelog permalink、重大なaxe違反なしを確認した。
- desktop 1280pxとmobile 390pxのafter screenshotを独立reviewし、表示中localeとutility配置を確認した。
- 独立reviewでblocking findingなし。basePathは`usePathname`とrouterの既定処理を維持し、二重prefixを加えない。

検証用`node_modules`、Next build出力、Playwright browserは結果確認後に再生成可能物として整理した。空き8 GiB下限を維持した。

## 後続作業

- home pageでdark modeをOFFにできない問題を別scopeで調査する。
- Nagiらしさが弱い見た目を別scopeで検討する。
- discord.py type referenceの要求は次段階として扱う。この修正へ混ぜない。
