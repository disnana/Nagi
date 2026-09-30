# ビルドと実行

Rust 1.98.1、Cargo 1.98.1、GCC 13.3を使用しました。依存の版はCargo.lockに記録されています。SQLiteはbundled Cソースをビルドするので、Cコンパイラも必要です。Linuxでのビルドと通信を検証しています。

```bash
cargo build --release --locked
./target/release/nagic run examples/hello.nagi
```

`check`はNagiの型・所有権検査、`lower`はLow生成、`build`はネイティブ生成、`run`は生成して実行です。Rustによる最後の借用・Send検査も含めて確かめるには`build`を実行します。`check`だけでRust backendの全診断を代替することはできません。

```bash
./target/release/nagic build examples/crud.nagi --out build/api --cost-report
./native-target/release/nagi-crud
```

成果物は指定したbuildディレクトリのLow、Rustソース、Cargo.tomlと、`native-target/release/`の実行ファイルです。compiler自身の`target`と生成プロジェクトのcacheは分けています。生成バイナリの出力先は`NAGI_NATIVE_TARGET_DIR`で変更できます。

Windowsネイティブで使う場合、ソースのビルドにはRust MSVC toolchainとCのビルド環境が必要です。WSL2ではLinux向けの手順を使用できます。Windowsネイティブでの検証は未実施です。
