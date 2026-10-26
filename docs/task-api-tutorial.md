# タスク管理JSON APIをゼロから動かす

この教材は公開版Nagi 0.2.1を使い、HTTPサーバーからSQLite CRUD、入力拒否、テスト、再起動後の保存、release buildまでを一続きで確認します。Rust/CargoとCのビルド環境を先に[セットアップ](getting-started.md)してください。

## 1. 最小のHTTPプロジェクト

新しい`my-nagi-api`フォルダーに2ファイルを置きます。

```text
my-nagi-api/
  nagi.toml
  main.nagi
```

`nagi.toml`:

```toml
entry = "main.nagi"
```

`main.nagi`（これだけで実行できる完全なプログラム）:

```nagi
import std.http.server as http

class State:
    greeting: str

async def home(request: http.Request, state: shared[State], policy: unit) -> Result[http.Response, Error]:
    return http.text(http.response_context(view(request)), http.Status.OK, view(state.greeting))

async def main() -> Result[unit, Error]:
    app = http.app_default[State](State(greeting="Hello, Nagi!"))
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), home)
    rate = try http.rate_policy(1000, 1000)
    app = try http.rate_route(app, http.Method.GET, "/", rate)
    options = http.default_options()
    options = try http.authority(options, "https://localhost", ["127.0.0.1:8094"], 1, 256)
    return await http.serve(app, 8094, options)
```

プロジェクト内で検査し、開発profileで起動します。

```sh
nagic --version
nagic check --project .
nagic run --project . --profile dev
```

別のterminalで`curl http://127.0.0.1:8094/`を実行すると`Hello, Nagi!`が返ります。`http.authority`は受理するHost/authorityを明示します。この例はloopback HTTP/1用で、インターネット公開のTLS終端ではありません。

## 2. 完成したSQLite CRUDへ進む

リポジトリの完全なサンプルは次の構成です。

```text
test-nagi-code/web-demo/
  nagi.toml
  tasks.nagi
  models.nagi
  validation.nagi
  smoke_api.py
```

`tasks.nagi`にはroute、有限のHTTP予算、SQLite Pool/Tx、bind parameter、commit/rollbackがあります。`models.nagi`がJSONの型、`validation.nagi`がtitleの1〜240 UTF-8 byte制約を持ちます。ブラウザーUIと認証は含みません。

リポジトリのルートから検査します。

```sh
nagic check --project test-nagi-code/web-demo
```

開発用に起動します。portを変える場合は、受理するauthorityも同じ値にします。

```sh
NAGI_PORT=8091 \
NAGI_HTTP_AUTHORITY=127.0.0.1:8091 \
NAGI_DB=/tmp/nagi-tasks.sqlite \
nagic run --project test-nagi-code/web-demo --profile dev
```

Windows PowerShellでは次のように設定します。

```powershell
$env:NAGI_PORT = "8091"
$env:NAGI_HTTP_AUTHORITY = "127.0.0.1:8091"
$env:NAGI_DB = "$env:TEMP\nagi-tasks.sqlite"
nagic run --project test-nagi-code/web-demo --profile dev
```

## 3. APIを使う

代表的な操作:

```sh
curl http://127.0.0.1:8091/health
curl -H 'Content-Type: application/json' \
  -d '{"title":"Docsを読む","done":false}' \
  http://127.0.0.1:8091/api/tasks
curl http://127.0.0.1:8091/api/tasks
curl http://127.0.0.1:8091/api/stats
```

利用できるrouteは`GET /api/tasks`、`GET /api/tasks/{id}`、`POST /api/tasks`、`PUT /api/tasks/{id}`、`DELETE /api/tasks/{id}`、`GET /api/stats`です。JSONの型不一致、不正JSON、空または長すぎるtitleは400、存在しない取得は404になります。SQL値は`?`へ型付きparameterでbindし、request由来文字列をSQL構造として実行しません。

## 4. HTTP回帰テストと永続化

サーバーを起動したまま別terminalで実行します。

```sh
python3 test-nagi-code/web-demo/smoke_api.py --base-url http://127.0.0.1:8091
```

正常CRUD、JSON・型・長さの拒否、404、統計など21件を確認します。保存を確認するには、taskを作成してサーバーを停止し、同じ`NAGI_DB`で再起動して`GET /api/tasks/{id}`を呼びます。別のDB pathを指定すると別のデータになります。

## 5. 配布用にbuildする

日常の再実行には`--profile dev`を使い、配布候補は明示的にrelease buildします。

```sh
nagic build --project test-nagi-code/web-demo --profile release
```

`native:`行に実行ファイルが表示されます。release buildの性能・動作は配布対象OSで検査してください。初回はRust依存をコンパイルするため時間がかかり、Pythonのサーバー起動とは同じ条件ではありません。

2026-10-26に公開Nagi 0.2.1から新しいlockを作った検証では、依存解決された`uuid 1.29.0`がRust 1.89以降を要求しました。Rust 1.88では依存解決後に失敗し、当日のstable 1.99.1ではrelease buildと21件のHTTP検査が成功しました。新規環境では現在のstable Rustを使ってください。既存`Cargo.lock`のあるbuildと新規解決では必要版が変わることがあり、Nagi 0.2.1は古いtoolchain向けの依存を自動で固定しません。

## 本番へ進む前の境界

- 標準serverはloopback HTTP/1です。外部公開では、信頼するreverse proxyでTLS、Host、body上限、接続上限、timeoutを設定し、Nagi側のauthorityと予算も合わせます。
- このtask APIは認証を実装していません。ローカルの学習・検証用です。[認証サンプル](../test-nagi-code/library-examples/http-auth/README.md)も配線例であり、productionのidentity systemではありません。
- 実運用の認証には、token verifier、issuer/audience/expiry、鍵更新、失効、user管理、認可規則、監査が必要です。固定demo credentialをproductionへ持ち込まないでください。
- SQLite Poolは1 connectionの例です。負荷・同時write・backup・migration・障害復旧を自分の条件で検証します。
- Nagi 0.2.1の標準HTTP APIにはWebSocket、HTTP/2、streaming、server-side TLSがありません。

同じAPIをPythonで書いた場合は[FastAPI・Quartとの比較](task-api-comparison.md)へ進んでください。

