# Supervisorを使う予約worker

午前と午後の予約を独立したactorで処理します。各workerは10席から始まり、重複した予約番号や空席を超える予約を独自の`BookingError`で返します。業務上の失敗では予約状態を保ちます。

```sh
nagic run --project test-nagi-code/application-examples/seat-reservations/nagi.toml
```

正常終了時のアプリ出力:

```text
reservations: validated duplicates, capacity, restart and sibling isolation
```

アプリ内で予約成功、重複、満席を検証したあと、午前workerに意図的な実行エラーを起こします。Supervisorによる再起動を有限回の問い合わせで確認し、午後workerの予約が保たれていることも確かめます。

予約はメモリだけに保存します。再起動したworkerの予約は失われ、10席に戻ります。永続化や決済を扱う予約システムではありません。

`smoke.py`は実行時間に上限を設け、終了コードと出力を確認します。業務上の失敗とworkerの実行失敗の区別は、Nagi側のassertで検証します。共通の検証コマンドは[上のREADME](../README.md)を参照してください。
