# NagiとVS Code拡張の正式リリース

mainでバージョンを上げたとき、`Nagi checks`の検証成功後にGitHub Releasesへ正式版を公開します。NagiとVS Code拡張は別々に判定します。Docsだけの変更や、バージョンを変えないコード更新ではリリースしません。

## バージョンを更新する

小さな修正ごとにバージョンは上げず、利用者向けの変更をまとめてリリースします。通常の不具合修正、CI・テスト・Docsの改善は現在のバージョンのままmainへ取り込み、利用者向けの差分を[CHANGELOG.md](../../CHANGELOG.md)の`Unreleased`へ記録します。公開する内容と検証結果が揃った段階で、対象のバージョンを更新するPRを作ります。その際に`Unreleased`を版と日付の見出しへ移します。

配布中の版が使えない不具合やセキュリティ修正など、早い配布が必要な場合は個別に判断します。NagiとVS Code拡張を同じタイミングで上げる必要はありません。

| 対象 | 更新する場所 | タグ | 配布物 |
|---|---|---|---|
| Nagi | ルート`Cargo.toml`の`workspace.package.version`と`Cargo.lock` | `nagi-vX.Y.Z` | Windows x64のZIP、Linux x86_64・macOS Apple Silicon・macOS Intelのtar.gz、それぞれのSHA-256 |
| VS Code拡張 | `editors/vscode-nagi/package.json`の`version` | `vscode-vX.Y.Z` | VSIXとSHA-256 |

正式版のバージョンは`X.Y.Z`です。現在の値より大きい値を使います。Nagiのバージョンを更新したら`cargo check --locked`を確認し、lockfileの更新が必要なら`cargo check`で更新してから`cargo check --locked`を行います。Docsの現在バージョンとインストール例も合わせて更新します。

PRをmainへマージすると、push前後のコミット全体を比較します。バージョン更新の後にDocsのコミットが続く複数コミットのpushも判定できます。main以外へのpush、PR、手動のCI実行では正式版を公開しません。最初のpushや、この仕組みを追加しただけで、変えていないNagiバージョンの初回リリースを作ることもありません。

## リリース画面の変更内容と差分

GitHub Releasesの本文には、配布物のインストール案内、対象版の手書き変更内容、前回からの差分、PRとコントリビューター情報をこの順で載せます。変更内容の元は、**公開するコミットの**[CHANGELOG.md](../../CHANGELOG.md)です。作業中のファイルや`Unreleased`を代わりに使いません。見出しは`## Nagi 0.1.8 / VS Code 0.1.10`のように版を明記します。Nagiだけ・VS Codeだけの見出しや、末尾に` — YYYY-MM-DD`を付けた見出しも使えます。各対象版には空でない項目を一つ用意します。正式な公開日時はGitHubの`published_at`に記録されます。

差分の基準は、同じcomponentの対象版より前の公開済み正式版です。draft・prerelease・他componentのタグは使いません。タグから今回のコミットへの比較と、そのタグが指すコミットから今回のコミットへの比較を両方載せます。GitHubの[自動生成リリースノート](https://docs.github.com/en/repositories/releasing-projects-on-github/automatically-generated-release-notes)にも同じ前回タグを明示し、merged PRsや標準のNew Contributorsを添えます。別途メールアドレスを収集・公開しません。

そのcomponentの初回公開では、自動生成が他componentのタグを暗黙に選ばないよう、generatorを呼びません。初回であることと公開コミットのsource snapshotを示し、存在しない比較タグを作りません。CHANGELOGの欠落・対象版の欠落や重複・空の項目、履歴取得やノート生成の失敗では、タグ作成や公開へ進みません。

## 検証と公開の流れ

1. バージョン差分を判定する。同時にLinuxで既存の型・所有権・ランタイム・HTTP・エディターの検証と、リリース条件の回帰テストを実行する。
2. 判定が終わった対象の配布物を、Linuxの検証と並行してビルドする。NagiはWindows x64、Linux x86_64、macOS Apple Silicon、macOS IntelでCLI・配布先から外部プロジェクトを使うテスト・エディターテストを確認する。Rustのhost architectureを確認し、別のCPU向けとして誤って配布しない。
3. Nagiのアーカイブをチェックアウト外に展開し、版・ヘルプをビルド環境なしで表示する。展開フォルダーをPATHに追加し、同梱物と別の場所にあるプロジェクトをビルド・実行する。WindowsではPowerShell版、Linux/macOSではbash版のインストーラーも検証する。
4. SHA-256を作り、Actionsの成果物へ保存する。
5. Linuxの検証と対象の配布物の検証がすべて成功したmainのバージョン更新時だけ、検証したコミットに新しいタグを作り、draft releaseへファイルをアップロードする。内容を読み直してSHA-256を照合した後、正式版として公開する。

PRでもバージョンを更新した対象の配布物を検査します。リリーススクリプト・インストーラー・CI定義を変更した場合は、バージョンを変えていない配布物も検査用に生成します。この検査だけではGitHub Releasesへ公開しません。

拡張のコード・発行者・パッケージ構成を変更した場合も、検証用VSIXを`release-vscode`成果物へ保存します。READMEやテストだけの変更では生成しません。発行者、拡張名、バージョンがVSIXのXMLとpackage.jsonで一致することを検査します。

公開用ジョブだけが`contents: write`を持ち、GitHub Actionsの組み込みtokenを使います。追加の公開tokenやMarketplaceアカウントは不要です。GitHub Pagesの自動公開は別の`Nagi website`ワークフローです。

Rustの依存取得とビルド結果をキャッシュします。OS・CPU・Rustのバージョンごとに分け、Cargo.lockとソースの変更で更新します。配布物の展開テストでも`native-target`を使い、依存パッケージの再コンパイルを減らします。キャッシュを復元した場合も、配布物とプロジェクトは新しい一時フォルダーに置き、そこにあるランタイムからHelloをビルド・実行します。標準の出力先は、別の配布テストで各OS上の実際のファイル配置を確認します。

PRやmain以外のブランチでは、新しい実行が始まると同じPR・ブランチの古い実行を取り消します。mainの実行はコミットごとに分け、別バージョンの公開を取り消さないようにします。

## 配布物の内容

Nagiのアーカイブには、**検証したコミットのGit管理下にあるランタイムのソース**を使います。`.git`、未追跡のメモ、ローカルのビルド・DB・ログは入りません。コミット情報は`release.json`で確認できます。

配布物は直下の`nagic`または`nagic.exe`、ビルドに必要な`runtime/`、`LICENSE`、短い日英の`README.txt`、`release.json`です。ランタイムのCargo manifestは開発用workspaceから独立させ、配布と同じ版を明記します。コンパイラのソース、開発用テスト、測定ログ、サイト・Docsのソースは含めません。

展開後の検査では、リポジトリ外のプロジェクトからPATH経由で実行します。Helloのほか、Futureという名前のclassを返す同期関数の差し替えと、sharedフィールドを持つclassのJSON変換・読み戻しを確認します。

アーカイブ全体を展開し、展開フォルダーをPATHに追加します。`runtime/`との位置を保てば`NAGI_ROOT`は通常不要です。NagiアプリのビルドにはRust/CargoとCのビルド環境が必要です。Linux版はUbuntu runnerと同等のglibc環境、macOS版はmacOS 15を想定します。0.1.5以前の配布物では実行ファイルは`target/release/`にあります。

[`install.ps1`](../install.ps1)と[`install.sh`](../install.sh)は、指定を省くとGitHubのLatestに指定されたNagiの公開版とSHA-256を取得します。公開処理はNagiだけをLatestに指定し、VSIXでは変更しません。インストーラーも転送先が`nagi-vX.Y.Z`でなければ停止します。版を指定する場合は`-Version X.Y.Z`／`--version X.Y.Z`です。READMEのコマンドはmainのインストーラーを使い、再実行で更新できます。

Windowsは`%LOCALAPPDATA%\Nagi\versions\current`のjunctionを固定のPATH入口にします。実行ファイルは`nagic.exe`なので、エディターから直接起動できます。既存の版ごとのPATH登録を整理し、Linux/macOSは`~/.local/bin/nagic`のリンクを切り替えます。切り替え後の起動検証に失敗したら元へ戻します。同時に2つのインストーラーを実行することはできません。

更新が成功してから、同じ保存先の旧版を公開アーカイブと照合し、ファイル・ディレクトリの構成と内容が一致するものだけ削除します。使う1版だけ残し、バックアップ版は常設しません。0.1.6の旧インストーラーからの移行にも同じ照合を使います。照合用のダウンロード失敗、追加・変更・使用中のファイルなどで削除できない場合は、更新自体は成功とし、残した場所を表示します。別の場所の手動展開物は触りません。

PATHの永続変更を省く場合は`-NoPath`または`--no-path`を使います。保存先を変えた場合は更新時も同じ引数を渡してください。更新前にはビルドを止め、旧版の絶対パスをVS Codeへ指定していた場合は`nagi.compilerPath`を空欄の自動探索か新版の絶対パスに変更します。Rust、C環境、VSIXは別途インストールします。

VSIXにはコンパイラを含みません。VS Codeの「VSIXからのインストール」で入れ、`nagic`を別にビルドするかNagiの配布物を用意します。[拡張の設定](../../editors/vscode-nagi/README.md)を参照してください。

## Visual Studio Marketplace

GitHubのVSIX公開とMarketplace公開は別です。現在のCIはMarketplaceへ送信しません。

Marketplaceにも公開する場合は、CIで検証したVSIXを使います。`editors/vscode-nagi`で、公開権限のある既存の認証を使い、次を実行してください。

```powershell
vsce publish --packagePath ../../build/distribution/nagi-language-0.1.13.vsix
```

ActionsまたはGitHub Releasesから取得した場合は、そのVSIXの保存先を指定します。`vsce publish patch`は版番号を追加で上げるため、ここでは使いません。

## 再実行と失敗時

失敗した場合はActionsで原因を確認し、**Re-run failed jobs**を使います。これなら成功したビルドの同じ成果物を再利用します。draftに同じ内容のファイルがある場合は保持し、足りないファイルだけを追加します。異なる内容の既存ファイルは上書きしません。

再実行は元のコミットのスクリプトを使います。公開スクリプト自体の修正は、修正版を含む新しいバージョンのPRから公開してください。既存draftは認証済みのリリース一覧から探します。新規draftは作成APIの応答からIDを取得し、そのIDへアップロード・検証・公開を行います。作成直後に一覧へ反映されていなくても、そのdraftを再検索する必要はありません。

公開済みの同じタグが同じコミットを指し、配布物とSHA-256が一致する場合、再実行は公開済みのファイルを保持して終了します。別コミットを指すタグや壊れた公開済み配布物は、勝手に置き換えずエラーにします。修正は新しいバージョンで公開します。古いNagiのビルドが遅れて完了した場合も、より新しいNagiの最新版表示を戻しません。

公開済みの本文も保持し、再実行時にCHANGELOGや自動ノートを読み直して書き換えることはありません。未公開のdraftに古い本文が残っている場合だけ、新しい本文へ更新し、APIの応答と読み戻した内容を確認します。本文と配布物の検証が終わるまでdraftのままです。

## 手元の検証

```bash
python -m unittest discover -s scripts/releases -p 'test_*.py'
```

このテストはインストーラーの正常動作・再実行・チェックサム不一致・アーカイブ経路・既存コマンドの保護、Docsだけの更新、独立したバージョン更新、複数コミットのpush、公開済みタグと本文の保護、draftの再開、Git管理外ファイルの除外を確認します。ノートの検査では、コミット済みCHANGELOG、componentごとの比較範囲、初回公開、変更操作前の失敗、draft本文の読み戻しも確認します。実際のGitHubへの公開は行いません。
