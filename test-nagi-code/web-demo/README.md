# Nagi Tasks JSON API

[English](README.en.md)

NagiとSQLiteで動く、タスク管理JSON APIの例です。作成・編集・完了・削除、集計を備えています。HTTP routeは標準APIで明示登録し、それぞれpublic policyを指定しています。ブラウザーUIと認証は含みません。

## 起動

リポジトリのルートで実行します。

```powershell
cargo build --release --locked -p nagic
.\target\release\nagic.exe run --project test-nagi-code/web-demo
```

既定のURLは[http://127.0.0.1:8091](http://127.0.0.1:8091)です。`NAGI_PORT`でportを、`NAGI_DB`でDB pathを変えられます。プロジェクトとして起動すると、既定で`test-nagi-code/web-demo/nagi-tasks.sqlite`を使い、再起動後も保存内容を読みます。既存DBを引き継ぐ場合は`NAGI_DB`に絶対pathを指定してください。

`GET /`はJSON APIの案内をplain textで返します。以前のraw HTML UIは標準HTTP routeから配信しません。`index.html`は型付きHTML応答が利用可能になるまで未接続の資料です。HTML markupをplain text responseとして扱わず、APIはJSONのまま提供します。

Windows x64の配布用exeを作る場合：

```powershell
.\scripts\build_windows_demo.ps1 -Source test-nagi-code/web-demo/tasks.nagi
.\build\distribution\nagi-tasks.exe
```

依存がキャッシュ済みなら`-Offline`を付けられます。配布exeとSQLite・VC++ runtimeの構成はbuild scriptを参照してください。実行後の保存データは別のSQLite fileになります。停止はターミナルでCtrl+Cを押してください。

## API

| メソッド・path | 内容 |
|---|---|
| `GET /health` | `ok`を返す明示public route |
| `GET /api/tasks` | 最新100件を一覧 |
| `GET /api/tasks/{id}` | 1件取得 |
| `POST /api/tasks` | `{"title":"やること","done":false}`を登録 |
| `PUT /api/tasks/{id}` | titleとdoneを置換 |
| `DELETE /api/tasks/{id}` | 削除し、削除件数を返す |
| `GET /api/stats` | 全件の件数と完了件数 |

タイトルはUTF-8で1〜240 byteです。SQLite tableにも制約があります。認証、browser UI、taskの優先度や期限は含みません。

## コードを読む

| ファイル | 内容 |
|---|---|
| `nagi.toml` | CLIとVS Codeで共通の入口 |
| `tasks.nagi` | 標準HTTP App、public policy付きroute、HTTP handler |
| `models.nagi` | JSONとDBの型 |
| `validation.nagi` | 入力検証 |
| `index.html` | SF04 typed HTML対応まで未接続の旧UI案 |

## 起動済みのserverを確認

```powershell
python test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

PythonはHTTP request/responseの照合だけを行います。自分で登録したtest dataをAPI経由で削除し、既存のtaskを残します。serverの起動・停止やDB fileの操作は行いません。
