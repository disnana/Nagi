# S1 Task handleの先行契約

この入力はADR 012の採用仕様をテストへ落としたもの。現行Nagiで使えるサンプルではない。private bridgeの実証と、Nagiのchecker/生成の完成は別である。

`contracts.json`にHighと独立した手書きLowを対で登録した。全Tの正常出口義務、一回消費、move alias、再代入、分岐・loop、scope所属、旧spawnと同名ユーザー関数を対象にする。negativeの期待はchecker段階・診断の意味・元のprimary行で固定する。現在のparse/import拒否はnegative成功に数えない。

`shadow`は既存のlet重複禁止を維持する例で、shadow機能を追加する要求ではない。Taskを上書きする`reassignment`の未受取義務とは分ける。scope内returnの解禁や一般Result bindingのmust-useも追加しない。

```sh
python scripts/verify_task_handle_contract_inputs.py
cargo test --locked -p nagic --example task-contract-red
cargo run --locked -p nagic --example task-contract-red -- --report /tmp/task-contracts.json
```

最初のコマンドは登録だけ、二つ目はrunnerが誤った段階や位置を成功扱いしない検査。三つ目は各契約を現行compilerへ照合し、一つでも未達ならexit 1、runnerの入出力失敗ならexit 2、ICEならpanic失敗となる。未実装の時点ではREDとなる。この失敗をskipや期待変更で隠さない。

checker/生成への接続後はこのrunnerのREDを解消し、保存Lowの再check・封印・実Cargo/native・診断対応を専用conformance harnessへ移す。ここはparse/checkまでの先行入力であり、std-onlyの既存conformanceへTokio stubを入れてruntime保証を作らない。
