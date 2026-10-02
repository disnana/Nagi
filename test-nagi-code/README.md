# Nagiのサンプル

## 自作ライブラリとRustの資産

[library-examples/](library-examples/)に、共通の料金計算を使うCLIとJSONレポート、serde_json、Tokio、独自HTTP基盤、Lowの差し替えの6プロジェクトがあります。[一覧と起動手順](../docs/library-examples.md)、[English](library-examples/README.en.md)を参照してください。

## 成功と失敗を分ける小さなAPI

[result-api/README.md](result-api/README.md) は、数値変換とSQLiteの1件取得から始めるサンプルです。Resultの`match`で、入力不正・対象なし・DB失敗・代替データへの回復を書き分けます。Pythonのsmokeは自分で起動したサーバーへHTTPリクエストを送り、応答だけを照合します。

## 画面付きのタスク管理

[web-demo/README.md](web-demo/README.md) に起動・exe配布・API確認の手順があります。ブラウザーでタスクを追加・編集・完了・削除でき、画面とAPIの両方をNagiサーバーが提供します。HTMLはexeへ埋め込みます。

## Rustライブラリを使う

`rust-bridge/bridge.nagi` は別ファイルの宣言をimportし、Rustで書いたCRC-32と`serde_json`を呼び出します。

```powershell
.\target\release\nagic.exe run --project test-nagi-code/rust-bridge
```

[../docs/modules-and-rust.md](../docs/modules-and-rust.md) にファイル分割と型付きRust連携の仕様をまとめています。VS Code拡張は [../editors/vscode-nagi/README.md](../editors/vscode-nagi/README.md) を参照してください。

## 在庫管理API

`inventory.nagi` は商品名と在庫数をSQLiteに保存するサンプルです。型付きのJSON入力、`view`による借用、`Result`と`try`による失敗の伝播、非同期DB操作、SQL集計をまとめて使います。既存の `hello.nagi` は小さな入門サンプルとして残しています。

## 動かす

リポジトリのルートで実行します。Rust/CargoとSQLiteをビルドできるCコンパイラが必要です。

```powershell
cargo build --release --locked
.\target\release\nagic.exe check test-nagi-code/inventory.nagi
# この設定を省略すると、終了時に消えるメモリDBを使います。
$env:NAGI_DB = "build/inventory-demo.sqlite"
$env:NAGI_PORT = "8090"
.\target\release\nagic.exe run test-nagi-code/inventory.nagi --cost-report
```

Linux / WSLでは次のように実行できます。

```bash
cargo build --release --locked
NAGI_DB=build/inventory-demo.sqlite NAGI_PORT=8090 \
  ./target/release/nagic run test-nagi-code/inventory.nagi --cost-report
```

DBの親ディレクトリは事前に存在する必要があります。上記では `nagic` が作る `build/` を使います。`--cost-report` は静的なコスト箇所のレポートであり、実行時の割り当て回数や性能測定ではありません。生成されたLowは `build/inventory/generated.low` に出力されます。

別のターミナルから呼び出します。以下はbashの例です。

```bash
curl -s http://127.0.0.1:8090/health
curl -s -H 'Content-Type: application/json' \
  -d '{"name":"ノート","quantity":12}' http://127.0.0.1:8090/items
curl -s http://127.0.0.1:8090/items/1
curl -s -X PUT -H 'Content-Type: application/json' \
  -d '{"name":"ノート","quantity":0}' http://127.0.0.1:8090/items/1
curl -s http://127.0.0.1:8090/inventory/summary
curl -s -X DELETE http://127.0.0.1:8090/items/1
```

PowerShellならJSONをUTF-8のbyte列にして送信できます。

```powershell
$body = [System.Text.Encoding]::UTF8.GetBytes('{"name":"ノート","quantity":12}')
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:8090/items `
  -ContentType 'application/json; charset=utf-8' -Body $body
Invoke-RestMethod http://127.0.0.1:8090/inventory/summary
```

## APIの振る舞い

| メソッド・パス | 結果 |
|---|---|
| `GET /items` | 最新100件をIDの降順で返す |
| `GET /items/{id}` | 1件取得。存在しなければ404 |
| `POST /items` | `name`、`quantity`から登録。200と登録した商品を返す |
| `PUT /items/{id}` | 名前と在庫数を両方置換。存在しなければ404 |
| `DELETE /items/{id}` | `{ "id": 1, "deleted": true }`。再削除は200で`false` |
| `GET /inventory/summary` | 全件の商品数、在庫数合計、在庫ゼロの商品数 |

商品は `{ "id": 1, "name": "ノート", "quantity": 12 }`、集計は `{ "item_count": 1, "total_quantity": 12, "out_of_stock": 0 }` の形です。

名前はUTF-8で1〜120 byte、在庫数は0〜1,000,000の整数、IDは正の整数です。日本語の「あ」は3 byteなので40文字まで入ります。名前の前後の空白は除去しません。空白だけの名前や同じ名前の重複登録も許可します。

不正なJSON、余分なフィールド、型違い、範囲外の値は400です。アプリとDBの両方で値の制約を持ち、SQLの値はすべてbindします。DB・内部エラーは詳細を伏せた500として返します。空のDBの集計はすべて0になります。

## 実HTTPで確認する

Python 3の標準ライブラリだけで動きます。テストは起動済みサーバーへHTTPリクエストを送り、ステータスとJSONを照合します。まず、空のメモリDBでサーバーを起動します。

```powershell
$env:NAGI_DB = ":memory:"
$env:NAGI_PORT = "8090"
.\target\release\nagic.exe run test-nagi-code/inventory.nagi
```

Linux / WSLなら `NAGI_DB=:memory: NAGI_PORT=8090 ./target/release/nagic run test-nagi-code/inventory.nagi` で起動します。別のターミナルでテストを実行してください。

```powershell
python test-nagi-code/smoke_inventory.py --base-url http://127.0.0.1:8090
```

登録・取得・更新・削除、UTF-8の境界値、不正入力、SQLのbind、集計、最新100件の一覧を確認します。Python側ではサーバーの起動・停止・再起動やファイル操作を行いません。テストで登録したデータはサーバーに残るため、再実行する場合はメモリDBのサーバーを起動し直してください。

## 現行の言語仕様に合わせた範囲

DBのinsert/updateは現在`str`と`i32`の2値を渡す形式なので、その形式で表現できる在庫モデルにしています。DBテーブルは既存のサンプルと衝突しにくい `sample_inventory_items` です。

一覧のページ送り、認証、名前検索、部分更新、スキーマ移行は含みません。更新は最後の書き込みが優先されます。ランタイムはloopbackで待ち受け、リクエスト処理の制限は2秒、bodyの上限は1 MiBです。タイムアウトしても受け付け済みのDB処理は完了する場合があります。

## exe単体で見られるフラクタル

`fractal.nagi` はマンデルブロ集合とジュリア集合をコンソールに描くデモです。関数、条件分岐、ループ、固定幅の整数と浮動小数点数、標準入出力を使います。ブラウザーやサーバーの起動、外部データは不要です。

Windows x64向けの配布用exeを作るには、リポジトリのルートで次を実行します。

```powershell
.\test-nagi-code\build_fractal_exe.ps1
# Cargoの依存がキャッシュ済みなら -Offline も指定できます。
```

配布するファイルは `build/distribution/nagi-fractal.exe` です。ビルドにはRust/CargoとMSVCのCビルド環境が必要ですが、配布先ではRust/Cargo・Nagi・Pythonのインストールは不要です。VC++ランタイムを静的リンクします。

exeを起動すると図形が出ます。`1`でマンデルブロ集合、`2`でジュリア集合を表示し、`q`または空のEnterで終了します。`@`は80回の反復で発散しなかった点、薄い文字ほど早く発散した点です。コンソール幅は80文字以上にしてください。入力が不正ならメニューを再表示します。

```powershell
.\build\distribution\nagi-fractal.exe
```

ソースから通常の方法で試す場合は `.\target\release\nagic.exe run test-nagi-code/fractal.nagi` でも動きます。
