# ADR 008: 登録資源の契約を一つの根拠へ集める

状態: 承認済み段階計画のPhase 3。先行テストの設計を採用。集約実装はまだ変更していない。

## 問題

登録資源のCopy・shared・field storage・Debugと、型引数の保持関係が複数の表や呼出し先へ分かれている。checkerと封印時の生成planが同じ登録identityから判断できるよう、資源の契約を集める。Nagi全型のcapability solverやRustのtrait体系を作り直す段階ではない。

## 採用

登録Resourceごとの単一static descriptorを私有ResourceContractとする。既存ResourceInfoを内包し、resource_infoはその共有参照を返す。公開ResourceInfoのfield集合・型・戻り値形は維持する。required fieldの追加も外部struct literalを壊すため行わない。新旧の独立したcapability表を残さない。

型引数は次の役割に分ける。

- inline payload: TurnのS/R/E。値のlayoutにも関係する。
- shared payload: AppのS、SupervisorのC。標準API内部のArc保持を含む。
- indirect protocol: ActorのM/R/E。channelやEnvelopeを介して使われる実値で、phantomではない。
- callback signature: AppのE。関数の署名として使われ、値fieldと同じ遍歴はしない。
- nominal phantom: GrantのP。permissionのidentityでありPの値を保持しない。

inline indexesはContract.info.inline_type_argumentsへ一度だけ、残りのindexesはContractの分類sliceへ一度だけ登録する。role queryはこの分類集合から導き、legacy inline/shared queryも同じsliceを参照する。独立したrole配列、const filter用bufferやmacroは作らない。全indexがarity未満で、各generic positionが一度だけ分類されることを検査する。未分類をnonpayload扱いへdefaultしない。

用途別adapterは維持する。layoutはinlineだけを辿る。auth proofとfield storageは、その用途で現在対象にしているinline/shared payloadを辿る。任意native内部の全payloadを解析する契約ではない。native Debugは資源のformatter契約で判定し、generic argument全体のDebugを要求しない。ActorのM/R/EとTurnのR/EのCharge適格性は型valid側で検査する。

enumの現Serde拒否、Chargeの深さ制限、auth proof探索のiterative walkは維持する。storageはclass等の所有fieldへの格納であり、永続DB格納や一般的な資源の寿命を保証しない。operationのPassing・borrow_owner・Handler/Mapperはoperationごとの登録を使い、資源のCopy属性から推測しない。

lifecycleはUnspecifiedとして不活性にする。Pool/Tx、task transfer、cleanupの新しい保証はPhase 4を待つ。利用者が任意Rust型へ安全契約を宣言する公開APIは追加しない。

## 先行テスト

現22resource・47operation・32fieldの値を、独立した手書き期待へ固定する。集合完全性、generic arity、Passing arity、役割indexの範囲・重複・欠落を別々に確認する。

小さいHTTP/Actor/dataのLow・Rust全文goldenを保存する。実resolverへ固定logical ModuleIdを渡すcfg(test) fixtureを使い、metadata・alias・deriveを削るnormalizerは作らない。既存の実ファイルHigh/保存Lowの未正規化一致、実Cargo/native、negativeの拒否段階・元位置は維持する。固定logical IDのgoldenは物理source provenanceの証明ではない。

Borrow/Mapperは既存HTTP AUTHだけでは観測できないため、小さい追加fixtureを用意する。runtimeが必要なclass/resourceをstandalone rustc corpusへ入れない。既存Cargo harnessを再利用する。新targetは4 OSの明示一覧にも登録し、harness登録を実行の証拠と混同しない。

test-only commitのCI成功後にだけ集約実装へ進む。Copy深さ63/64/65のcheckerとsealed deriveの差は、別の有限probeで確かめる。未解決の差を正常golden、skip、backend拒否allowlistへ入れない。受理・interop derive等を変える必要があれば、根拠と代替案を分けて判断する。

## 見送る案

- 全capabilityに共通の再帰walker: 署名・phantom・共有state・間接protocolの扱いが用途ごとに異なる。
- 新Contractへboolean条件をコピーして旧表も残す: 二重定義の場所が増える。
- 今回class全体のCopy・Serde・Charge・深さ制限まで同時に再設計する: 資源metadata集約とは影響と判断が異なる。
- 一般trait/effect/region solver、全面Typed IR、標準runtime/APIの交換: 今回の集約に必要な根拠がない。

## 完了の観測

現行pass/fail、拒否段階と元位置、Low/Rust bytes、実行結果を維持する。資源capabilityとpayload roleの登録根拠が一つになり、emitterが意味論を再推測しない。全関連CIとbounded conformance/fuzzを確認する。変更に関係するfrontend時間・生成内容を比較し、runtime性能向上の主張には使わない。

参照: [段階計画](../compiler-rust-boundary-plan.md)、[言語契約](../language-invariants.md)、[ADR 006](006-sealed-codegen-input.md)、[テスト分類](../compiler-testing.md)。
