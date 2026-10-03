# Nagiの紹介・Docsサイト

紹介ページと日本語・英語Docsを、GitHub Pagesで配信できる静的HTMLにします。サーバー側の処理や外部フォント、アクセス解析は使いません。紹介ページには名前の由来、動くコード例、現在の実装範囲を載せています。

日本語のDocs本文は`docs/`などの既存Markdown、英語版は`docs/en/`を読みます。公開サイトは`https://nagi.disnana.com/`、英語版は`/en/`です。各ページの言語指定とmetadataを設定し、右上の言語リンクで対応するページへ切り替えます。生成時にサイト内のリンクへ変換し、リンク先・見出し・CSS・JavaScriptの存在を確認します。ソースコードやサンプルのフォルダーへのリンクはGitHubへ移動します。

## 手元で確認する

Python 3.12以降を使います。コマンドはリポジトリのルートで実行してください。

```bash
python -m venv build/website-venv
build/website-venv/bin/python -m pip install -r website/requirements.txt
build/website-venv/bin/python website/build.py --base-path / --out build/website-preview
python -m http.server 4173 --directory build/website-preview
```

Windowsでは、venv内のPythonを`build\website-venv\Scripts\python.exe`に置き換えます。

ブラウザーで`http://localhost:4173`を開きます。紹介、Docsの目次、記事、モバイルの目次、コードのコピーを確認できます。サーバーはCtrl+Cで停止します。

公開用の生成は、次のコマンドです。出力先は`build/website/`です。

```bash
build/website-venv/bin/python website/build.py
```

公式リポジトリでは`website/CNAME`のドメインを使い、サイトの起点を`/`にします。生成物にも`CNAME`を含めます。Pagesワークフローと手元のビルドは同じ設定を使います。

forkでは公式の`CNAME`を使わず、`--repository https://github.com/所有者/リポジトリ名`から通常のGitHub Pages URLと`/リポジトリ名/`を選びます。`所有者.github.io`リポジトリなら起点は`/`です。`--base-path`と`--site-url`を明示すると設定を上書きできます。公式リポジトリをproject Pages用に生成する場合は、`--base-path /Nagi/ --site-url https://disnana.github.io`を指定します。

## GitHub Pagesを公開する

`.github/workflows/pages.yml`を公開するリポジトリの既定ブランチへ入れ、次の設定と実行を行います。

1. GitHubの **Settings → Pages → Build and deployment** でSourceを **GitHub Actions** にする。
2. **Actions → Nagi website → Run workflow** を開く。
3. 既定ブランチを選んで実行する。
4. `deploy`の成功後、表示されたPagesのURLを開く。

PRではサイトをビルド・検査し、HTMLをActionsの成果物へ保存します。**サイト関連の変更がmainへ入ると、検査成功後に自動デプロイします。** 初回公開や再デプロイには、mainを選んでRun workflowを使えます。公開設定、リポジトリのvisibility、custom domainはこのワークフローでは変更しません。

生成HTMLを別の方法で配信する場合も、`build/website/`の内容を使えます。GitHub Pagesへの配信でJekyllが処理しないように`.nojekyll`を含めています。

## 内容を更新する

| 内容 | 更新するファイル |
|---|---|
| 紹介文、名前の由来、開発状況 | 日本語は`website/templates/home.html`、英語は`home.en.html` |
| 紹介のコード例 | `website/examples/double.nagi`。実行して出力を確認する |
| Docs本文 | 日本語は元の`docs/*.md`や各サンプルのREADME、英語は`docs/en/*.md` |
| Docsの分類・掲載ページ | `website/build.py`の`GROUPS`・`ENGLISH_GROUPS`と`EXTRA` |
| 文字、余白、配色、画面幅への対応 | `website/assets/site.css` |
| 共通メニュー、metadata | `website/templates/page.html` |

新しいDocsページを追加した場合は、日英両方の本文と目次に追加してください。片方の本文がない場合や掲載ページが一致しない場合はビルドを止めます。本文の更新時も翻訳を合わせて更新します。公開サイトに載せるページは明示的に選び、未登録のMarkdownを自動で掲載しない構成です。
