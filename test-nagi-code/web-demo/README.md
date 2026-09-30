# Nagi Tasks — ブラウザーから使う小さなタスク管理

HTML/CSS/JavaScriptを埋め込み、NagiのJSON APIとSQLiteで動くサンプルです。追加・編集・完了・削除、絞り込み、全件の集計を備えています。外部のWebライブラリやCDNは使いません。

## 起動

リポジトリのルートで実行します。

```powershell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run test-nagi-code/web-demo/tasks.nagi
```

ブラウザーで [http://127.0.0.1:8091](http://127.0.0.1:8091) を開いてください。`NAGI_PORT`でポートを、`NAGI_DB`でDBパスを変えられます。初期設定では起動時のフォルダーに`nagi-tasks.sqlite`を作り、再起動後も保存内容を使います。

Windows x64の配布用exeを作る場合：

```powershell
.\scripts\build_windows_demo.ps1 -Source test-nagi-code/web-demo/tasks.nagi
.\build\distribution\nagi-tasks.exe
```

依存がキャッシュ済みなら`-Offline`を付けられます。HTMLとSQLite・VC++ランタイムはexeに組み込まれるため、配布するファイルはexeひとつです。実行後の保存データは別のSQLiteファイルになります。Rust・Nagi・Pythonは配布先には不要です。停止はターミナルでCtrl+Cを押してください。

## コードを読む

| ファイル | 内容 |
|---|---|
| `tasks.nagi` | HTTP handlerと起動 |
| `models.nagi` | JSONとDBの型 |
| `validation.nagi` | 入力検証 |
| `index.html` | ブラウザー画面とAPI呼び出し |

画面は`GET /`、APIは`GET/POST /api/tasks`、`GET/PUT/DELETE /api/tasks/{id}`、`GET /api/stats`です。登録・更新bodyは`{"title":"やること","done":false}`です。タイトルはUTF-8で1〜240 byte、一覧は最新100件まで、集計は全件です。

このサンプルはloopbackで動くローカルアプリです。認証や本番公開の構成は含めていません。

## 起動済みのサーバーを確認

```powershell
python test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

PythonはHTTPリクエストと応答の照合だけを行います。自分で登録したテストデータはAPI経由で削除し、既存のタスクを残します。サーバーの起動・停止やDBファイルの操作は行いません。
