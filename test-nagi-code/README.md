# Nagiのサンプル

[English](README.en.md)

CLI、HTTP API、SQLite、actor、Rust連携の例です。Nagiで書く処理と、Rustに任せる処理を各READMEで説明しています。本番用のひな形ではなく、入力・出力と制限を確認するためのサンプルです。

リポジトリ一式を取得し、そのルートで実行してください。Nagi 0.1.9とRust/Cargo、OSごとのビルド環境が必要です。[セットアップ](../docs/getting-started.md)を参照してください。`main`からビルドしたコンパイラには、公開版と同じ版番号でも未リリースの修正が含まれる場合があります。

## 自作ライブラリとRustの資産

| サンプル | 内容 |
| --- | --- |
| [ライブラリとRust連携](library-examples/README.md) | 共有料金計算、module、独自エラー、標準HTTP、自作Axum基盤など10プロジェクト |
| [アプリケーション](application-examples/README.md) | JSON集計、SQLite API、DBなしの見積API、ファイル保存、監督付きworker |
| [手書きLowの注文見積もり](low-examples/order-quote/README.md) | 波括弧構文、Low同士のimport、入力検証 |
| [ローカルRust crate](rust-library/README.md) | `path`依存とCargo feature、Nagiの型への変換 |
| [Rustの橋渡し](rust-bridge/) | CRC-32、serde_json、非同期Rust関数 |
| [コードマップ](../examples/code-map/README.md) | 型・module・呼び出しの関係を図にする |

共通の検証手順は[アプリ一覧](application-examples/README.md)と[ライブラリ一覧](../docs/library-examples.md#開発時にまとめて確認する)にあります。

## 成功と失敗を分ける小さなAPI

[Result API](result-api/README.md)は、数値変換とSQLiteの1件取得を使い、入力不正・対象なし・DB失敗・代替データを`match`で書き分けます。

## 画面付きのタスク管理

[Nagi Tasks](web-demo/README.md)は、ブラウザーからタスクを追加・編集・完了・削除する例です。NagiがAPIと入力検証を担当し、ランタイムがHTTPとSQLiteを扱います。画面のHTML/CSS/JavaScriptは実行ファイルへ埋め込みます。

## Rustライブラリを使う

Rust側の関数とcrateをNagiから呼ぶ方法は、[Rust連携のサンプル](rust-library/README.md)と[importとRust連携](../docs/modules-and-rust.md)を参照してください。手書きLowを挟む必要はありません。

## 在庫管理API

[inventory.nagi](inventory.nagi)は商品名と在庫数をSQLiteへ保存します。NagiでJSONの型・入力検証・ルート・エラー処理を定義し、SQLの実行はランタイムへ任せます。SQL文字列の列名やDBスキーマは、通常の`check`では検査しません。

### 動かす

```sh
nagic run test-nagi-code/inventory.nagi
```

既定ではport 8090とメモリDBを使います。保存する場合は、存在するディレクトリ内のSQLiteファイルを指定します。

```powershell
$env:NAGI_DB = "inventory-demo.sqlite"
$env:NAGI_PORT = "8090"
nagic run test-nagi-code/inventory.nagi
```

```sh
NAGI_DB=inventory-demo.sqlite NAGI_PORT=8090 nagic run test-nagi-code/inventory.nagi
```

`--cost-report`を付けると静的なコスト箇所を表示します。実行時の割り当て回数や速度を測る機能ではありません。

### APIの振る舞い

| メソッド・パス | 結果 |
| --- | --- |
| `GET /items` | 最新100件をIDの降順で返す |
| `GET /items/{id}` | 1件取得。存在しなければ404 |
| `POST /items` | 名前と在庫数を登録。200と登録した商品を返す |
| `PUT /items/{id}` | 名前と在庫数を両方置換。存在しなければ404 |
| `DELETE /items/{id}` | IDと`deleted`を返す。再削除も200で`false` |
| `GET /inventory/summary` | 全件の商品数、在庫数合計、在庫ゼロの商品数 |

bashでは次のように登録できます。Windowsで`curl`がPowerShellのaliasになっている場合は`curl.exe`を使ってください。

```sh
curl -H 'Content-Type: application/json' \
  -d '{"name":"ノート","quantity":12}' http://127.0.0.1:8090/items
curl http://127.0.0.1:8090/inventory/summary
```

### 実HTTPで確認する

保存用のサーバーを停止し、空のメモリDBで起動し直します。PowerShellでは次を実行してください。

```powershell
$env:NAGI_DB = ":memory:"
nagic run test-nagi-code/inventory.nagi
```

bashでは`NAGI_DB=:memory: nagic run test-nagi-code/inventory.nagi`で起動します。別のターミナルで検証します。

```sh
python test-nagi-code/smoke_inventory.py --base-url http://127.0.0.1:8090
```

Python 3の標準ライブラリだけで、登録・取得・更新・削除、UTF-8の境界値、不正入力、bind、集計、一覧を確認します。サーバーの起動・停止やDBファイルの操作は行いません。テストデータが残るため、再実行時はメモリDBのサーバーを起動し直してください。

### 現行の言語仕様に合わせた範囲

名前はUTF-8で1〜120バイト、在庫数は0〜1,000,000、IDは正の整数です。空白の除去や重複名の拒否はしません。JSONの余分なフィールド・型違い・範囲外は400、DB・内部エラーは詳細を伏せた500です。SQLの値はbindし、テーブルにも値の制約を付けています。

現在のinsert/updateは`str`と`i32`を渡す固定の形式です。ページ送り、認証、名前検索、部分更新、スキーマ移行は含めていません。更新は最後の書き込みが優先されます。loopbackで待ち受け、handler期限は2秒、本文上限は1 MiBです。タイムアウトしても受け付け済みのDB操作は完了する場合があります。

## exe単体で見られるフラクタル

[fractal.nagi](fractal.nagi)はマンデルブロ集合とジュリア集合をコンソールへ描きます。数値計算、ループ、標準入出力の例で、サーバーやDBは使いません。

```sh
nagic run test-nagi-code/fractal.nagi
```

`1`でマンデルブロ集合、`2`でジュリア集合、`q`または空のEnterで終了します。コンソール幅は80文字以上にしてください。

Windows x64の配布用exeは、リポジトリのルートで次のように作れます。

```powershell
.\test-nagi-code\build_fractal_exe.ps1
.\build\distribution\nagi-fractal.exe
```

ビルドにはRust/CargoとMSVCのCビルド環境が必要です。VC++ランタイムを静的リンクするため、配布先ではRust/Cargo・Nagi・Pythonは不要です。依存を取得済みなら`-Offline`も指定できます。
