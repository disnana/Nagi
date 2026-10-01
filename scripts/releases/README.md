# NagiとVS Code拡張の正式リリース

mainでバージョンを上げたとき、`Nagi checks`の検証成功後にGitHub Releasesへ正式版を公開します。NagiとVS Code拡張は別々に判定します。Docsだけの変更や、バージョンを変えないコード更新ではリリースしません。

## バージョンを更新する

| 対象 | 更新する場所 | タグ | 配布物 |
|---|---|---|---|
| Nagi | ルート`Cargo.toml`の`workspace.package.version`と`Cargo.lock` | `nagi-vX.Y.Z` | Windows x64のZIP、Linux x86_64・macOS Apple Silicon・macOS Intelのtar.gz、それぞれのSHA-256 |
| VS Code拡張 | `editors/vscode-nagi/package.json`の`version` | `vscode-vX.Y.Z` | VSIXとSHA-256 |

正式版のバージョンは`X.Y.Z`です。現在の値より大きい値を使います。Nagiのバージョンを更新したら`cargo check --locked`を確認し、lockfileの更新が必要なら`cargo check`で更新してから`cargo check --locked`を行います。Docsの現在バージョンとインストール例も合わせて更新します。

PRをmainへマージすると、push前後のコミット全体を比較します。バージョン更新の後にDocsのコミットが続く複数コミットのpushも判定できます。main以外へのpush、PR、手動のCI実行では正式版を公開しません。最初のpushや、この仕組みを追加しただけで、変えていないNagiバージョンの初回リリースを作ることもありません。

## 検証と公開の流れ

1. Linuxで既存の型・所有権・ランタイム・HTTP・エディターの検証と、リリース条件の回帰テストを通す。
2. 対象の配布物をビルドする。NagiはWindows x64、Linux x86_64、macOS Apple Silicon、macOS Intelでコンパイラとエディターテストを確認する。Rustのhost architectureを確認し、別のCPU向けとして誤って配布しない。
3. Nagiのアーカイブをチェックアウト外に展開し、その中のコンパイラでCPUサンプルを検査し、Hello Worldをビルド・実行する。
4. SHA-256を作り、Actionsの成果物へ保存する。
5. mainのバージョン更新時だけ、検証したコミットに新しいタグを作り、draft releaseへファイルをアップロードする。内容を読み直してSHA-256を照合した後、正式版として公開する。

PRでもバージョンを更新した対象の配布物を検査します。リリーススクリプトやCI定義を変更した場合は、バージョンを変えていない配布物も検査用に生成します。この検査だけではGitHub Releasesへ公開しません。

公開用ジョブだけが`contents: write`を持ち、GitHub Actionsの組み込みtokenを使います。追加の公開tokenやMarketplaceアカウントは不要です。GitHub Pagesの自動公開は別の`Nagi website`ワークフローです。

## 配布物の内容

Nagiのアーカイブは、**検証したコミットのGit管理下のソース**とコンパイラを含みます。`.git`、未追跡のメモ、ローカルのビルド・DB・ログは入りません。コンパイラは`target/release/nagic`または`nagic.exe`、コミット情報は`release.json`にあります。

アーカイブ全体を展開し、`runtime/`を含むフォルダー構成を保って使います。コンパイラのビルドを省けますが、NagiのアプリをビルドするにはRust/CargoとCのビルド環境が必要です。Linux版はGitHubのUbuntu runnerでビルドするため、同等のglibc環境を想定します。macOS版はmacOS 15のrunnerで検証します。Apple Siliconは`macos-arm64`、Intelは`macos-x86_64`のファイルを選びます。

VSIXにはコンパイラを含みません。VS Codeの「VSIXからのインストール」で入れ、`nagic`を別にビルドするかNagiの配布物を用意します。[拡張の設定](../../editors/vscode-nagi/README.md)を参照してください。

## 再実行と失敗時

失敗した場合はActionsで原因を確認し、**Re-run failed jobs**を使います。これなら成功したビルドの同じ成果物を再利用します。draftに同じ内容のファイルがある場合は保持し、足りないファイルだけを追加します。異なる内容の既存ファイルは上書きしません。

再実行は元のコミットのスクリプトを使います。公開スクリプト自体の修正は、修正版を含む新しいバージョンのPRから公開してください。draftは認証済みのリリース一覧から探し、アップロード後はリリースIDで読み直して検証します。

公開済みの同じタグが同じコミットを指し、配布物とSHA-256が一致する場合、再実行は公開済みのファイルを保持して終了します。別コミットを指すタグや壊れた公開済み配布物は、勝手に置き換えずエラーにします。修正は新しいバージョンで公開します。古いNagiのビルドが遅れて完了した場合も、より新しいNagiの最新版表示を戻しません。

## 手元の検証

```bash
python -m unittest discover -s scripts/releases -p 'test_*.py'
```

このテストはDocsだけの更新、独立したバージョン更新、複数コミットのpush、公開済みタグの保護、draftの再開、Git管理外ファイルの除外を確認します。実際のGitHubへの公開は行いません。
