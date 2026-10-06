# S1 public runtime の検証記録

対象branch: `feat/task-result-bridge-stage1`。開始head: `5c2c8282fbd9841cd3b0a2a78e9742d7ec3a811e`。commit/push/merge/releaseは行っていない。

- `01-public-api-red/`: 公開4型が未定義の実装前snapshotと外部crateテスト入力。`cargo-test-network.log`はE0432・exit 101。`cargo-test.log`は最初のsandbox network不足のinfra記録で、契約REDではない。依存indexのretryを止めてnetwork追加実行へ切り替えたため初回processのexitは143。
- `02-public-migration-first.log`: prototypeを公開本体へ移した初回16oracle成功。以後、fault種別のinternal seam観測も残してError出口を検査した。
- `03-public-green/`: 中間snapshot・runtime全体成功・unit costの初回debug測定。最終snapshotとして扱わない。
- `04-public-final/`: 最終runtime本体・公開APIテスト・production-layout benchmark・検査ログ。runtime 206 unit成功、cost1 ignored、外部公開API3成功、doc9成功（うちTaskの4 negative）。ignored costは別の明示実行で成功し、通常の契約成功数へ足さない。

公開APIはcrate rootの `TaskScope`, `Task<T>`, `TaskFailure`, `TaskFailureKind`。旧 `Scope` と `concurrent.rs` は変更していない。private prototypeの実装は削除し、16oracleは `task::tests::` で本体を検査する。TaskScopeのspawnは旧Result[unit,Error]故障経路、spawn_valueはtyped業務値、receiveはactual join後のTaskFailure、discardはunit、join/cancelはactual join後のErrorを返す。TaskFailureはopaque・非Clone、Taskは非Clone・非Sync（Send維持）。Nagiのscope所属・must-consume・escapeはcompilerの担当であり、Rustの内部owner tokenだけで保証しない。

各recordとreceive/discardは対象ticketだけを退役する。drainの終端で外部Dropされたjoin済みrecordを一回sweepする。primary故障時だけ未join taskへabortを要求し、関連故障ごとに全entryを再走査しない。新しい子はsticky故障があると登録時にabort要求を記録する。任意Tや終了Futureをrecordへ保存しない。

`native-cost-production-release.log`は通常buildの公開TaskScopeで、current-thread Tokio、release、既存metrics allocatorを使用する。5loop初回の短いサンプルで時間の揺れが大きかったため原ログを`*-first.log`へ保持し、25loop・7反復の結果を保存した。runtime/libのcfg(test) Fake NativeKeyは通常buildとkey layoutが異なるため、unitの`native-cost-unit-debug.log`のallocation/速度をproductionと混同しない。unitは内部の保持・capacityとfault cloneの補助観測に用いる。allocatorは呼出threadだけを数えるが、このfixtureの全childは同じcurrent-thread runtimeで動く。

1024件の64-byte payloadは、完了未joinで1024recordと65,536 payload bytes、join済み未受取でも同じpayloadを保持する。handleを外でDropするとpayloadは破棄され、1024軽量recordは次のjoin/drain sweepで0になる。HashMap capacityは残る（capacityはallocされた正確なbyte量ではない）。128件の既完了legacy faultではprimary1・related127・related Vec capacity128を保持する。内部Fault cloneはString分のallocationがあり、元ErrorのcauseはArcで共有する。alloc-free・ゼロコスト・独自GC・background joinを主張しない。

同期Dropはabort要求までで、actual join完了や外部副作用rollbackを保証しない。non-yielding処理・任意Rust Drop panicからの普遍回復は範囲外。検査targetはこのLinux x86_64のみで、4 OS CIとNagi compiler経由のnative検査はroot担当の別記録に従う。
