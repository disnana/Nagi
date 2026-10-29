# Nagi Docsサイト

Nagiの日本語・英語Docsは、Next.js 16とFumadocsで静的HTMLへ出力します。日本語は`/docs/`、英語は`/en/docs/`です。公開先は`https://nagi.disnana.com/`で、GitHub Pagesのproject path用に`/Nagi`を指定したbuildも検査します。

## 手元で確認する

Node.js 24以降を使います。repository rootから次を実行してください。

```bash
cd website
npm ci --ignore-scripts
npx playwright install chromium
npm run prepare:content
npm run dev
```

`http://localhost:3000/`を開きます。開発用表示と静的配信との差は完全検査で確認します。

```bash
cd website
npm test
```

`npm test`は型検査、content生成・整合性、静的build、route・link・検索index、実browserでのmobile・keyboard・検索・copy・accessibilityを検査します。静的生成物は`website/out/`です。個別には`npm run typecheck`、`npm run build`、`npm run verify:static`、`npm run verify:links`、`npm run test:browser`を使えます。

project Pagesと同じpathで確認する場合は、buildから検査まで同じ値を渡します。

```bash
cd website
PAGES_BASE_PATH=/Nagi npm run build
PAGES_BASE_PATH=/Nagi npm run verify:static
PAGES_BASE_PATH=/Nagi npm run verify:links
PAGES_BASE_PATH=/Nagi npm run test:browser
```

## 内容を更新する

| 内容 | 更新する場所 |
|---|---|
| 日本語の教材・reference | `website/content/docs/ja/` |
| 英語の教材・reference | `website/content/docs/en/` |
| sidebarの順序 | 各localeの`meta.json` |
| homeと共通navigation | `website/components/`、`website/app/` |
| 変更履歴 | rootの`CHANGELOG.md`が正本。生成処理が軽量indexと版別pageを作る |
| 旧URLとの対応 | `website/content/docs/{ja,en}/reference/legacy-routes.json`とcontent生成処理 |
| content・route・linkの検査 | `website/scripts/` |
| browser QA | `website/tests/` |

日本語と英語は同じpage setを維持します。英語不足を日本語へfallbackしないため、片方だけの追加は検査で失敗します。公開版、未公開の変更、設計案を区別し、動くと説明するcodeは対応版とbackendを明記してください。

`website/scripts/prepare-content.mjs`は選択した既存Docsと`CHANGELOG.md`を生成用contentへ変換します。生成物を直接編集せず、正本または変換規則を修正してください。

## GitHub Pages

`.github/workflows/pages.yml`はNode.js 24でlockfileから依存を導入し、教材のPython検査、型検査、static build、route/link、browser QAを実行します。PRでは成果物を保存し、site関連の変更が`main`へ入ったときだけ検査後にGitHub Pagesへdeployします。custom domain、DNS、repository visibilityはworkflowで変更しません。

custom domainの静的ファイルの正本は`website/public/CNAME`です。重複したCNAMEを別の場所へ置かないでください。
`PAGES_BASE_PATH=/Nagi`のproject Pages buildでは、公式domainをforkへ設定しないよう出力からCNAMEを除きます。`.nojekyll`はどちらにも残します。

`llms.txt`は公開Docsに加え、`ai/README.md`、Nagi開発Skill、デバッグ、言語、Rust連携の5つの公開AI資料だけを明示的に索引します。内部計画や監査ログは載せず、sourceが欠けたbuildは失敗します。

公式domainではbase pathを空にし、forkやproject Pagesでは`actions/configure-pages`が返すpathを`PAGES_BASE_PATH`としてbuildへ渡します。site内link、検索取得先、画像、canonical、hreflangは両方の形で検査します。

## 説明を見直すとき

code blockを先に置き、構文の説明、正確な実行commandと結果、実用例の順に案内します。feature matrixやsource linkだけで書き方の説明を置き換えません。日本語routeは日本語、英語routeは英語で統一し、API識別子はそのまま使います。

検索・AI向けにはcanonical URL、hreflang、sitemap、robots、scope済み`llms.txt`を生成します。これらは推薦や順位を保証しません。性能や安全性の主張には、測定条件または実装・testの根拠を付けてください。
