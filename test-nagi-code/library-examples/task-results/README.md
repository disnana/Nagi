# Taskの結果・move・業務Err

このディレクトリで`nagic check`、`nagic run`を実行します。手書きLowは`nagic run main.low`で実行できます。Taskはmainへ接続済みで、公開releaseにはまだ含まれません。

`original`のhandleと受取義務を`move`で移し、一度awaitして42を受け取ります。次のTaskは業務Errを返します。外側のTaskFailureと内側のResultを別にmatchするため、業務Errでも兄弟は終了まで動きます。unitを返す兄弟は明示discardします。discardは子の停止ではなく受取放棄で、scopeは実際の終了を待ちます。

`42`、`business sentinel`、`sibling finished`の後に`after scope`と`task-results: OK`が表示されます。兄弟の表示と業務Errの表示の前後は固定しません。TaskFailureを表示してもscopeは故障状態なので、最後の成功表示には到達しません。

保存Lowを使う場合は`nagic lower --out build/lowered`を実行し、`nagic run build/lowered/generated.low`で再読込します。実装の検証では元Highを削除したコピーでも保存Lowをbuild/runし、手書きLowとは別に確認します。

[Taskガイド](../../../docs/task-handles.md)・[English](README.en.md)
