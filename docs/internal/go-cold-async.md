# Go backend: cold user asyncの最小段階

この段階は、共有checkerが受理したユーザー定義の`async def` / `async fn`と、その
直接`await`をGo backendへ接続する。標準operation、task、scope、spawn、actor、HTTPは
対象外で、従来どおりGo toolchain起動前に拒否する。

## 保つ意味論

- 呼出し引数は呼出し側で左から一度だけ評価する。async本体はその後に作られる
  cold Futureの`await`まで開始しない。
- async関数の引数はFuture作成時の値としてclosureへ移す。Go emitterが引数式を
  再評価したり、本文開始まで呼出し側の変数名を読み直したりしない。
- `await`はFutureを一度だけ消費する。生成helperも二重消費をpanicで拒否し、本文が
  panicした場合に再実行できる状態へ戻さない。
- `Result` / `try await`、Highから生成したLow、保存Lowで同じ成功・失敗伝播を使う。
- async `main`は返されたcold Futureをentrypointが明示的にawaitしてから、従来の
  `Result`終了処理を行う。

Goで本文をgoroutineへ自動spawnしない。直接awaitだけの段階では同期的に本文を進める
ことで、asyncをCPU並列化と誤解させず、暗黙のtask・cancel・join契約を追加しない。

## 今回まだ保証しないもの

- 複数Futureの並行poll、task / scope / spawn、取消、timeout、実join
- async関数値をcallbackとして渡すこと、borrow / view capture、owner cleanup
- async標準operationとpublic `std.actor`、HTTP、DB、auth/session
- Go scheduler上の並列性能、Rust/Tokioとの速度・memory・順序の同一性

これらはprivate lifecycle kernelの後続gateである。対応していない標準operationを
同名のGo関数へ落としたり、Rust backendへ自動fallbackしたりしない。

## 検証

実Go build/runで、Highと独立に再parseした保存Low、async `main`、引数評価→本文→
`Result`成功値の順序、`try await`を確認する。標準async operationはGo toolchainを
欠損させた状態でも元source位置のunsupported診断になり、生成物を作らないことを
negativeで固定する。

この2件を加えたGo integrationの必須件数は、Linux/macOSが29、Windowsが28である。
Windowsだけ少ない1件は、既存のUnix symlink保護ケースであり、async検査のskipではない。
