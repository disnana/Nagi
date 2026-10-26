# FastAPI・Quart比較コード

Nagiの[タスク管理API](../../../test-nagi-code/web-demo/)と同じCRUD・入力拒否・SQLite再起動保存を、FastAPIとQuartで確認するための比較用コードです。performance benchmarkではありません。

2026-10-26にPython 3.13.16と[`requirements.txt`](requirements.txt)の固定版で検証しました。Quart 0.23.1にはPython 3.13以降が必要です。新しく作る環境では同日にsecurity修正版として公開されたPython 3.13.18以降を指定し、この検査を再実行してください。3.13.18は元の検証環境の`uv`配布一覧になく、実測は3.13.16です。

```sh
uv venv --python 3.13 .venv
.venv/bin/python --version
uv pip install --python .venv/bin/python -r requirements.txt
.venv/bin/python test_apps.py
```

Windows PowerShell:

```powershell
uv venv --python 3.13 .venv
.venv\Scripts\python.exe --version
uv pip install --python .venv\Scripts\python.exe -r requirements.txt
.venv\Scripts\python.exe test_apps.py
```

`test_apps.py`はFastAPIをUvicorn、QuartをHypercornで空いているloopback portへ起動し、実際のHTTPで同じ検査を行います。DBは一時folderに作り、各serverを停止・再起動して保存を確認します。

比較の解説は[同じタスクAPIをNagi・FastAPI・Quartで比べる](../../task-api-comparison.md)を参照してください。

