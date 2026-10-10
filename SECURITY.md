# セキュリティ方針

[English](SECURITY.en.md)

## 脆弱性を報告する

[GitHubの非公開報告フォーム](https://github.com/disnana/Nagi/security/advisories/new)から報告してください。GitHubへのログインが必要です。未修正の脆弱性の詳細や実証コードを、公開Issue・PRに投稿しないでください。

調査に必要なのは、対象のバージョンまたはcommit、OS、最小の再現手順、影響と成立条件です。秘密情報や第三者のデータは含めず、自分の環境で再現してください。

報告を調査し、修正と詳細の公開時期を報告者と調整します。返信や修正の期日は保証しません。通常の不具合や質問には[Issues](https://github.com/disnana/Nagi/issues)を使えます。

## 対象の範囲

Nagiのコンパイラ、ランタイム、VS Code・JetBrains拡張、配布物と公開処理を対象とします。まずmainと最新の正式リリースで影響を確認します。古い版に関する報告も受け付けますが、旧版への修正の適用は個別に判断します。

HTTP・JSONなどの外部入力による情報漏えい、意図しない実行、データの破損、サービス停止や、エディターの信頼設定を迂回する実行は、セキュリティ報告の対象です。依存ライブラリの問題は、Nagiで影響が出る経路と合わせて報告してください。

## 利用時の前提と制限

最新の正式公開版はNagi 0.1.11です。GitHub main `e609aba158921226a632f16d41eb8b0f4ad5aebd`は開発sourceで、正式0.2.0は未リリースです。公開0.1.11の配布物と契約は変更されません。`nagic build/run`やRust連携には、コードを隔離して実行する機能はありません。信頼できないソース、`nagi.toml`、Rust依存を試すときは、秘密情報や大事なファイルのない隔離環境を使ってください。

公開0.1.11には開発mainのrequest-bound `std.auth`契約、routeごとの明示`std.http.Policy`必須化、`std.db.sqlite.Query`/`Parameters`の新しい標準契約は含まれません。開発mainでは`AuthScope`/`Grant`をrequestへ結び、標準routeには明示Policyを要求します（`public`は明示的な匿名許可です）。標準SQLite経路はcanonical literal QueryとParametersによる構造/値bind境界を使います。これらは該当する標準API経路の制約であり、アプリpolicyの正しさ、tenant認可、任意Rust、全アプリの安全性を証明しません。SF02/SF03/SF04/SF06とSF07の横断budget acceptance、SF08は未完了で、正式0.2.0のreleaseもありません。

VS Codeはワークスペース、JetBrainsはプロジェクトの信頼設定を確認してからコンパイラを起動します。起動したプログラムは、通常のアプリと同じ権限で動きます。

開発mainの標準`std.http.server`は`127.0.0.1`にbindし、header・keep-alive待機、本文受信、handlerの期限と本文上限を設けます。接続・処理容量、送信・停止の期限も設定できます。公開0.1.11の旧`serve(Db, port)`とはAPI/設定/制限が異なります。各版の説明は[標準HTTPの制限](docs/http-server.md)と[旧APIの負荷試験](docs/http-capacity.md)を確認してください。

これらの制限は、アプリの認証・認可や処理の巻き戻しを提供しません。取消しても受理済みのDB書き込み等が完了する場合があります。開発mainの採用範囲と未完の作業は[RFC](docs/internal/security-foundation/rfc.md)と[実装計画](docs/internal/security-foundation/implementation-plan.md)、公開版との差は[CHANGELOG](CHANGELOG.md)に記載しています。

外部へ公開する場合は、前段のプロキシなどでTLS、接続数、header・idleの期限、流量を管理し、アプリの認証・認可を実装してください。回線を圧迫するDDoSには、ホスティング事業者やCDN側の対策も必要です。
