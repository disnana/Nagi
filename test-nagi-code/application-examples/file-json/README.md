# JSON設定ファイル

Nagiで型付きJSONを作り、UTF-8ファイルへ保存して読み直します。ファイル操作はRustの`std::fs`を使う短いアダプターに任せ、型とJSONの検証はNagiで書いています。追加のRust crateは使いません。

リポジトリのルートで実行します。`nagi.toml`にRustファイルを指定しているため、`--project`を付けてください。

```sh
nagic run --project test-nagi-code/application-examples/file-json
```

アプリの出力:

```text
{"site":"東京","enabled":true,"retries":3}
configuration.json
file-json: OK
```

`NAGI_SAMPLE_FILE`で保存先を指定できます。既定値は実行時のカレントディレクトリに置く`configuration.json`です。`nagic run --project`ではプロジェクトのディレクトリに作ります。

Rustの`create_new`を使い、既存のファイルは上書きしません。同じ保存先で再実行すると、理由を標準エラーに表示して終了コード1を返します。設定を更新する機能や、書き込み全体を原子的に行う仕組みは含みません。

`smoke.py`は日本語と空白を含むパスで保存・読込を確認し、既存のJSONや設定が保たれること、保存先がディレクトリなら失敗することも確認します。[共通の検証コマンド](../README.md)は一時ディレクトリを使います。
