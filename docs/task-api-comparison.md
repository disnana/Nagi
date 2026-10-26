# 同じタスクAPIをNagi・FastAPI・Quartで比べる

ここでは、SQLiteへ保存する同じJSON CRUDを実装し、選択時に違いが出る箇所を比べます。Nagiは言語とruntime、FastAPIとQuartはPythonのWeb frameworkです。この違いを含めて、アプリのserver・依存・配布までを比較します。速度ランキングではありません。

比較コード:

- Nagi 0.2.1: [`test-nagi-code/web-demo/`](../test-nagi-code/web-demo/)
- FastAPI 0.162.0: [`fastapi_app.py`](comparisons/task-api/fastapi_app.py)、Uvicorn 0.54.0
- Quart 0.23.1: [`quart_app.py`](comparisons/task-api/quart_app.py)、Hypercorn 0.18.0
- 共通のPython検査: [`test_apps.py`](comparisons/task-api/test_apps.py)

Python例はPython 3.13.16、HTTPX 0.28.1で2026-10-26に検証しました。Quart 0.23.1はPython 3.13以降を要求します。Python 3.13.18が同日にsecurity修正版として公開されているため、新しく環境を作る場合は3.13.18以降の修正版を使い、この検査を再実行してください。3.13.18はこの作業環境の`uv`配布一覧にまだなく、ここでの実測対象には含めていません。再現手順は[比較コードのREADME](comparisons/task-api/README.md)にあります。

## 揃えた動作

3実装とも、taskの作成・一覧・取得・更新・削除、完了数の集計、SQLiteへの保存を行います。入力は`title: str`と`done: bool`で、titleは1〜240 UTF-8 byte、IDは1〜`i64::MAX`です。値はSQLのplaceholderへbindします。Python共通検査は型・余分なfield・空title・unpaired surrogate・ID上限・存在しないID・再起動後の保存を確認します。Nagiの21件は型・余分なfield・空/長さ境界・不正JSON・ID 0・SQL bind・CRUDを確認し、unpaired surrogateと`i64::MAX + 1`を直接送る検査は含みません。

| 判断点 | Nagi 0.2.1 | FastAPI 0.162.0 | Quart 0.23.1 |
|---|---|---|---|
| 入力model | Nagi `class`を`json_decode[SaveTask]`へ渡す | Pydantic modelをhandler引数へ書く | request JSONをアプリで検査する |
| 入力エラー | handlerのErrorをroute mapperが400へ変換 | Pydantic検証は422。この例は不正Unicodeを反射しないgenericな422本文へ整形 | この例は`abort(400)` |
| SQLite | 標準のtyped Query/Parametersとasync Pool/Tx | Python `sqlite3`をsync handlerで使用 | event loopを止めないよう`asyncio.to_thread`で`sqlite3`を実行 |
| HTTP server | Nagi runtimeのloopback HTTP/1 | Uvicornを明示して起動 | Hypercornを明示して起動 |
| 開発時の確認 | `nagic check`後、必要時にCargo build | Python起動、Pydantic、tests。型検査toolは別途選ぶ | Python起動、tests。入力schemaと型検査toolは別途選ぶ |
| 配布 | 対象OS向けnative実行ファイルと必要なnative依存 | Python環境、app依存、Uvicorn | Python環境、app依存、Hypercorn |
| 初回準備 | Rust依存のcompileが必要 | Python wheel/packageのinstall | Python wheel/packageのinstall |

## コード上の違い

### Nagi

HTTPの受理authority、rate policy、response・queue・memory予算をアプリ初期化で明示します。SQLiteはPool/Txと型付きbindを使い、`Result`で失敗を伝播します。その分、routeごとのpolicyやtransaction終了をコードに書きます。現在の標準APIで扱える範囲と制約が強く、Python packageを直接importできません。

### FastAPI

Pydantic modelをhandler引数に書くと、JSON decode・schema・型/field検証とOpenAPIをframeworkがまとめて扱います。この例では厳密な文字列・boolと余分なfield拒否をmodelに書きました。SQLite accessは同期handlerで行い、productionの接続管理やmigrationは別のlibrary/設計を選びます。UvicornなどのASGI serverが必要です。

### Quart

Flaskに近いroute APIをasyncで使えます。今回のJSON schemaはアプリで明示的に検査しました。標準`sqlite3`は同期処理なので、Quartの案内に従いevent loopからthreadへ逃がしています。schema libraryやasync database libraryを追加すれば記述は変わります。deploymentにはHypercornなどのASGI serverが必要です。

## どれを選ぶか

Nagiを試す理由は、Python風の見た目だけではありません。Nagi側の型・所有権・Result・capability検査を使い、Rustのnative配布やcrate adapterと組み合わせたい場合に評価できます。一方、Python ecosystem、豊富なmiddleware、OpenAPI中心の開発、短い起動準備を重視するならFastAPIが強い候補です。Flaskに近いAPIでasync applicationを組みたい場合はQuartを比較できます。

Nagi 0.2.1には標準PostgreSQL、WebSocket、HTTP/2、streaming、server-side TLSがありません。標準HTTP serverはloopback用で、外部公開ではreverse proxyが必要です。FastAPI/Quartにもproduction server、TLS/proxy、timeout、body/connection制限、認証、DB pool、migration、監視の設計が別途必要です。

## 検証したことと、主張しないこと

- 公開Nagi 0.2.1で`check`、開発server、21件のHTTP smoke test、同じSQLite fileを使った再起動後の取得を確認しました。
- Python 3.13.16で固定したFastAPI/UvicornとQuart/Hypercornを実socketで起動し、同じCRUD・型/UTF-8/ID境界・404・再起動後の取得を確認しました。起動検査はprocessごとのnonceを照合し、同じportにある別serviceを成功とみなしません。
- コード行数はframeworkの既定機能、明示した安全境界、コメント、整形で変わるため、開発効率の点数には使いません。
- 同一負荷・同一machineの性能benchmarkは実施していません。native生成だけを根拠に、NagiがFastAPI/Quartより高速・省memory・安全だとは主張しません。

公式資料: [FastAPI request bodyとPydantic](https://fastapi.tiangolo.com/tutorial/body/)、[FastAPI SQL database tutorial](https://fastapi.tiangolo.com/tutorial/sql-databases/)、[FastAPI serverの手動起動](https://fastapi.tiangolo.com/deployment/manually/)、[Quart quickstart](https://quart.palletsprojects.com/en/latest/tutorials/quickstart/)、[Quartで同期処理を扱う方法](https://quart.palletsprojects.com/en/latest/how_to_guides/sync_code/)、[Quart deployment](https://quart.palletsprojects.com/en/latest/tutorials/deployment/)、[Python 3.13.18の公開情報](https://blog.python.org/2026/10/python-3152-31410-31318-31216-31118/)。
