# コンパイラ・Rust境界の未決事項

## Q-001: 同じアプリの識別と実行ファイルの世代を分ける

状態: 判断待ち。PR0で発見。コード・テスト期待は変更していない。

### 問題

新依頼のPhase 2は、成功generationをside-by-sideで保持し、実行中の旧binaryを上書きしないことを要求する。一方、既存テストは同じcanonical source/outを再buildした際、実行ファイルのcanonical pathが前回と同じであることを要求する。

依頼のStop条件「既存テストの意味論上の期待値を変更しないと通らない」に該当するため、先に判断を求める。旧assertを削ったり、generationを単なるsymlinkで同じ実体へ向けて隔離したことにしたりしない。

### 根拠

- [shared_target.rs](../../compiler/tests/shared_target.rs)の212行: 等価pathで再buildした成功binaryのcanonical pathを前回と比較する。230行: 既定/明示で同じcacheを選ぶ別buildにも同一pathを要求する。
- 同テストの195〜199行: stemにapp identityの16桁だけを要求する。世代名を追加すればこの期待も調整が必要。
- [project.rs](../../compiler/tests/project.rs)の351〜359行: `build/native-target/release`直下と現在のname形を検査する。
- [ADR 005](adr/005-native-artifact-identity.md)の「契約と採用案」: source/outの識別と生成位置維持を定める。ただし「同じ生成先への同時compile未対応」は#76の範囲・現在の制約であり、単独で将来機能を禁止する規範とは扱わない。
- [projects](../projects.md)は既存生成Cargo.lockの保持、`--out`と`build/<入口>/`の生成場所を説明する。generation移行でこれを捨ててはいけない。

### 選択肢

| 案 | 挙動・影響 |
|---|---|
| A. app identityを維持し、実行artifactだけ世代固有にする | `native:`は毎回成功generationの実path。等価source/out・既定/明示cacheは同じapp ID、別generationのpathは異なる。旧binaryは維持。依存cache・project cwd・既存out/lockは維持する設計を検証する |
| B. 同一canonical binary pathを維持する | 旧exeをそのpathで実行している場合、新exeの置換とWindows互換性に衝突する。run終了までbuildを止める等が必要となり、今回のgeneration条件を満たせない。Phase 2の条件の再検討が必要 |

### 互換性と必要なテスト変更

Aでは実行ファイルpath/nameの既存期待が変わる。app identityの決定性は保ち、generation identityとは別に検査する。`native:`を読むscripts/editor/distributionを確認し、固定nameを前提にする箇所を移行する。生成Low・Rust・Cargo.tomlの既存参照先と既存Cargo.lockの継承は別の契約として保持する。

現在のstdout・異なるappの分離・同一dependency cache・project cwdのassertは残す。追加するのは同一appの旧exe継続、別generation、並行build、failed buildでlatest不変、Windowsでの実行中exeと新buildである。

同一pathのassertをapp ID同一＋generation path相違へ変えるのは、実装ミスを隠すtest weakeningではなく、新しいgeneration契約への移行である。ただし今回の明示Stop条件に従い、回答を受けるまで変更しない。

### 推奨案

Aを推奨する。旧app identityとcache共有を保ち、実際にrunするimmutable generationを明示できる。成功metadataはatomic更新し、run中の旧generationを上書き・削除・killしない。

判断後は[計画書](compiler-rust-boundary-plan.md)のPhase 2互換性、ADR 005との差、新しいacceptanceを確定する。Phase 1から順に再開し、Phase 2を先に実装しない。

## 後続Phaseで具体化する項目

以下はまだ値・APIを決めていない。現時点の実装や追加保証とは扱わない。

| 項目 | 判断する時点 | 条件 |
|---|---|---|
| Pool容量・acquire/busy timeout・transaction開始mode | Phase 4設計 | 新しい公開policy値が必要ならStop。既存Dbの値を新Poolへ暗黙に流用しない |
| Pool/Tx module・API、worker session/lease | Phase 4設計 | SQLite候補を既存依存で検証。unsafe/追加driver/依存が必要ならStop |
| 旧Grant[P]とGrant[P,Scope]の互換性 | Phase 5以降 | Phase 4完了前に実装しない。arity変更・既存API削除は別途判断 |
| 任意opaque Rust resource・async callback | 計画外 | Rust API自動importや自己申告Contractを追加しない。必要なら別設計 |
| Txを捕捉したFutureのtask transfer | Phase 4設計・negative tests | Futureの戻り値型だけで判定しない。alias/return/Option/標準task起動を含むprivate capture factsを検証。一般effect/regionが必要ならStop |
| 世代snapshotと互換出力の実装 | Phase 2設計・failing tests | canonical outのwrite lock、app別metadata、check/lower並行とprojection途中失敗を観測。外部workspace全体のatomic snapshotは追加しない |
