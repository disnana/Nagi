# Nagiの紹介・Docsサイト

紹介ページと日本語Docsを、GitHub Pagesで配信できる静的HTMLにします。サーバー側の処理や外部フォント、アクセス解析は使いません。紹介ページには名前の由来、動くコード例、現在の実装範囲を載せています。

Docs本文は`docs/`などの既存Markdownを読みます。サイト用に本文を複製する必要はありません。生成時にサイト内のリンクへ変換し、リンク先・見出し・CSS・JavaScriptの存在を確認します。ソースコードやサンプルのフォルダーへのリンクはGitHubへ移動します。

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

標準のURLは`https://disnana.github.io/Nagi/`です。別のリポジトリで使う場合は、`--base-path /リポジトリ名/`、`--site-url https://所有者.github.io`、`--repository https://github.com/所有者/リポジトリ名`で変えられます。ユーザーサイトの`所有者.github.io`リポジトリなら`--base-path /`にします。

## GitHub Pagesを公開する

`.github/workflows/pages.yml`を公開するリポジトリの既定ブランチへ入れ、次の設定と実行を行います。

1. GitHubの **Settings → Pages → Build and deployment** でSourceを **GitHub Actions** にする。
2. **Actions → Nagi website → Run workflow** を開く。
3. 既定ブランチを選んで実行する。
4. `deploy`の成功後、表示されたPagesのURLを開く。

PRとmainへのpushではサイトをビルド・検査し、HTMLをActionsの成果物へ保存します。**公開は既定ブランチでの手動実行時だけ**です。Docsを更新したあとも、公開したい時にRun workflowを実行します。公開設定、リポジトリのvisibility、custom domainはこのワークフローでは変更しません。

生成HTMLを別の方法で配信する場合も、`build/website/`の内容を使えます。GitHub Pagesへの配信でJekyllが処理しないように`.nojekyll`を含めています。

## 内容を更新する

| 内容 | 更新するファイル |
|---|---|
| 紹介文、名前の由来、開発状況 | `website/templates/home.html` |
| 紹介のコード例 | `website/examples/double.nagi`。実行して出力を確認する |
| Docs本文 | 元の`docs/*.md`や各サンプルのREADME |
| Docsの分類・掲載ページ | `website/build.py`の`GROUPS`と`EXTRA` |
| 文字、余白、配色、画面幅への対応 | `website/assets/site.css` |
| 共通メニュー、metadata | `website/templates/page.html` |

新しいDocsページを追加した場合は、目次にも追加してください。公開サイトに載せるページは明示的に選び、未登録のMarkdownを自動で掲載しない構成です。
