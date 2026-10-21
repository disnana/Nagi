# 初回CIのfixtureパス不一致

対象HEAD: `c9c923dcdcc2bdd3c62704c0cbb65578baf69176`、project tooling run [39606221465](https://github.com/disnana/Nagi/actions/runs/39606221465)。Linux 3.10/3.12は成功、macOS/Windowsは同じalias testで失敗した。検証器を広げず、compilerが出すcanonical依存pathとfixtureを揃える。Linuxで出力とreceiptに異なる2 aliasを設け、旧fixtureで同じValueError、新fixtureで成功を確認した。必須assert・検査・期限を変更しない。

## Job 119242972945

```text
2026-10-21T13:14:37.2619370Z ERROR: test_schema3_lexical_receipt_directory_alias_is_supported (test_build_receipt.ReceiptTests.test_schema3_lexical_receipt_directory_alias_is_supported)
2026-10-21T13:14:37.2620060Z ----------------------------------------------------------------------
2026-10-21T13:14:37.2620430Z Traceback (most recent call last):
2026-10-21T13:14:37.2626350Z   File "/Users/runner/work/Nagi/Nagi/scripts/tests/test_build_receipt.py", line 336, in test_schema3_lexical_receipt_directory_alias_is_supported
2026-10-21T13:14:37.2627920Z     MODULE.verify(aliased)
2026-10-21T13:14:37.2628320Z   File "/Users/runner/work/Nagi/Nagi/scripts/verify_build_receipt.py", line 625, in verify
2026-10-21T13:14:37.2628890Z     _verify_generated_units(receipt_path, receipt, app.rsplit("-", 1)[-1])
2026-10-21T13:14:37.2629680Z   File "/Users/runner/work/Nagi/Nagi/scripts/verify_build_receipt.py", line 551, in _verify_generated_units
2026-10-21T13:14:37.2630300Z     body = visit(digest, None)
2026-10-21T13:14:37.2630560Z            ^^^^^^^^^^^^^^^^^^^
2026-10-21T13:14:37.2630990Z   File "/Users/runner/work/Nagi/Nagi/scripts/verify_build_receipt.py", line 546, in visit
2026-10-21T13:14:37.2631490Z     visit(match[1], child, dependency["path"], depth + 1)
2026-10-21T13:14:37.2632000Z   File "/Users/runner/work/Nagi/Nagi/scripts/verify_build_receipt.py", line 516, in visit
2026-10-21T13:14:37.2632470Z     _v3_reference(reference, manifest.parent)
2026-10-21T13:14:37.2632980Z   File "/Users/runner/work/Nagi/Nagi/scripts/verify_build_receipt.py", line 470, in _v3_reference
2026-10-21T13:14:37.2633480Z     raise ValueError("schema 3 unit dependency path mismatch")
2026-10-21T13:14:37.2633860Z ValueError: schema 3 unit dependency path mismatch
2026-10-21T13:14:37.2634100Z 
2026-10-21T13:14:37.2634240Z ----------------------------------------------------------------------
2026-10-21T13:14:37.2634570Z Ran 36 tests in 0.838s
2026-10-21T13:14:37.2634710Z 
2026-10-21T13:14:37.2634800Z FAILED (errors=1, skipped=1)
2026-10-21T13:14:37.2745330Z ##[error]Process completed with exit code 1.
2026-10-21T13:14:37.2907570Z Post job cleanup.
2026-10-21T13:14:37.4293490Z [command]/opt/homebrew/bin/git version
```

## Job 119242973121

```text
2026-10-21T13:14:34.2723934Z ERROR: test_schema3_lexical_receipt_directory_alias_is_supported (test_build_receipt.ReceiptTests.test_schema3_lexical_receipt_directory_alias_is_supported)
2026-10-21T13:14:34.2724851Z ----------------------------------------------------------------------
2026-10-21T13:14:34.2725134Z Traceback (most recent call last):
2026-10-21T13:14:34.2729740Z   File "D:\a\Nagi\Nagi\scripts\tests\test_build_receipt.py", line 336, in test_schema3_lexical_receipt_directory_alias_is_supported
2026-10-21T13:14:34.2730111Z     MODULE.verify(aliased)
2026-10-21T13:14:34.2730377Z   File "D:\a\Nagi\Nagi\scripts\verify_build_receipt.py", line 625, in verify
2026-10-21T13:14:34.2730649Z     _verify_generated_units(receipt_path, receipt, app.rsplit("-", 1)[-1])
2026-10-21T13:14:34.2730970Z   File "D:\a\Nagi\Nagi\scripts\verify_build_receipt.py", line 551, in _verify_generated_units
2026-10-21T13:14:34.2731207Z     body = visit(digest, None)
2026-10-21T13:14:34.2731357Z            ^^^^^^^^^^^^^^^^^^^
2026-10-21T13:14:34.2731551Z   File "D:\a\Nagi\Nagi\scripts\verify_build_receipt.py", line 546, in visit
2026-10-21T13:14:34.2731789Z     visit(match[1], child, dependency["path"], depth + 1)
2026-10-21T13:14:34.2732031Z   File "D:\a\Nagi\Nagi\scripts\verify_build_receipt.py", line 516, in visit
2026-10-21T13:14:34.2732242Z     _v3_reference(reference, manifest.parent)
2026-10-21T13:14:34.2732474Z   File "D:\a\Nagi\Nagi\scripts\verify_build_receipt.py", line 470, in _v3_reference
2026-10-21T13:14:34.2732745Z     raise ValueError("schema 3 unit dependency path mismatch")
2026-10-21T13:14:34.2732950Z ValueError: schema 3 unit dependency path mismatch
2026-10-21T13:14:34.2733074Z 
2026-10-21T13:14:34.2733157Z ----------------------------------------------------------------------
2026-10-21T13:14:34.2733343Z Ran 36 tests in 1.294s
2026-10-21T13:14:34.2733415Z 
2026-10-21T13:14:34.2733479Z FAILED (errors=1, skipped=3)
2026-10-21T13:14:34.3839921Z ##[error]Process completed with exit code 1.
2026-10-21T13:14:34.4049211Z Post job cleanup.
2026-10-21T13:14:34.5182106Z [command]"C:\Program Files\Git\bin\git.exe" version
```
