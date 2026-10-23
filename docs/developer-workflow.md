# 開発・デバッグの早見表

Nagi 0.2.1で「書く → 検査 → 実行 → 配布」を進める入口です。初めてなら[インストール](getting-started.md)、採用を検討するなら[特徴と他言語との比較](comparison.md)へ。

## 普段はこの3つ

`nagi.toml`があるアプリのディレクトリで実行します。

```sh
nagic check --project .
nagic run --project . --profile dev
nagic build --project . --profile release
```

| 操作 | 分かること・使う場面 |
|---|---|
| `check` | Rustをコンパイルせず、Nagiの構文・型・所有権などを検査。編集後の最初の確認に使う |
| `run --profile dev` | 開発用にビルドして実行。実行だけが目的なら直前の`build`は不要 |
| `build --profile release` | 配布・性能測定に使う最適化ビルド。実行時の動作確認も別途行う |

CLIの`build`/`run`はprofileを省略するとreleaseです。devでも初回はRust依存のコンパイルが必要です。通常の整数overflowはdevでpanic、releaseでwrapするため、本番条件も確認してください。[profileと差分ビルドの詳細](development-builds.md)

## 知りたいことから探す

| 目的 | 最初に読むもの |
|---|---|
| 関数が何を受け取り、何を返すか | [組み込みAPI一覧](builtins.md)、[型](types.md)、[エディター](editor.md) |
| HTTP APIを書く | [HTTP入門](http.md) → [サーバーAPI](http-server.md) → [認証・認可](security.md) |
| DBを使う | [SQLite Pool・transaction](sqlite-pool.md)、[SQL検査](sql-check.md) |
| エラーを返す・処理する | [Resultとエラー処理](error-handling.md) |
| move/borrowを直す | [所有権](ownership.md)、[viewの用例](view-and-zero-copy.md) |
| Rust crateを使う | [プロジェクト設定](projects.md)、[Rust連携](modules-and-rust.md) |
| 動くコードから始める | [サンプル一覧](library-examples.md)、[最初のCLIアプリ](first-app.md) |
| ビルドが遅い | [時間・再ビルド理由の読み方](development-builds.md)、[性能測定の条件](performance.md) |
| アプリのCIを用意する | [GitHub Actions生成](user-ci.md) |
| AIに開発・デバッグを任せる | [AI向け入口](../ai/README.md)、[開発Skill](../ai/skills/nagi-development/SKILL.md) |
| Nagi本体を直す | [貢献ガイド](contributing.md)、[リポジトリの作業規約](../AGENTS.md) |

## エラーを切り分ける

| 症状 | 次に確かめること |
|---|---|
| `check`で失敗 | 最初の診断にある元ソースの位置、型、import、move/viewを確認する |
| `check`成功後に`build`で失敗 | Cargoの最初のエラー、Rust adapter、依存、toolchainを確認。Nagiで受理したコードの生成不具合もあり得る |
| 実行結果がおかしい | 同じ入力・作業ディレクトリ・profileで再現し、業務Err、panic、起動失敗を区別する |
| IDEだけ動かない | IDEに指定したcompilerの絶対パスと`nagic --version`を確認。compilerとpluginの版は別 |
| 設定が読まれていない | 明示したSOURCEだけでは近くの設定を探さない。`nagic check main.nagi --project .`のように指定する |

`check`成功、Rustビルド成功、アプリのテスト成功は別の結果です。生成Low/Rustを直接直すと次回生成で失われます。元ソースやadapterを修正してください。再現・共有する情報の整理は[デバッグ手順](../ai/debugging.md)にあります。

## ビルド時間を測る

```sh
nagic run --project . --profile dev --build-stats
```

標準エラーの`nagi-build-stats`でCargo前の時間、Cargoの時間、Fresh/compiled件数を確認できます。初回・変更なし・関数変更を分けて測り、同じprofileで比較してください。`--incremental`/`--incremental=units`はdev専用の明示指定です。小さなアプリや変更内容によっては遅くなるため、[測定結果と対応範囲](development-builds.md)を確認して選びます。

`NAGI_NATIVE_TARGET_DIR`は依存の共有先、`--out`は生成物の保存先です。成功実行ファイルは`native:`に表示されたパスを使います。調査の最初に全cacheを消すと再現情報と再利用できる依存が失われます。[読み取り専用のcache調査](projects.md)から始めてください。

## リリース前

完全な`check`、releaseビルド、アプリ固有の正常系・異常系テストを実施します。Nagi 0.2.1に汎用の`nagic test`コマンドはありません。既存のテストharnessをCIへ組み込み、HTTPなら実リクエスト、DBなら保存結果まで確認します。インストール済み版と機能の対応は[変更履歴](../CHANGELOG.md)と[公開Release](https://github.com/disnana/Nagi/releases)で確認できます。0.2.2は開発中です。
