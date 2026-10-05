# 保存するcompiler regression corpus

期待値は[内部言語契約](../../docs/internal/language-invariants.md)と公開referenceに照らして決める。実装のacceptや既存testの成功だけで仕様へ昇格しない。runner/縮小/CIの使い方は[compiler-testing.md](../../docs/internal/compiler-testing.md)。

| 保存例 | 回帰させない境界 | 再利用元の既存test |
| --- | --- | --- |
| i64_boundaries | context無しのliteralがi32へ縮まらずi64 MIN/MAXを保つ | compiler/tests/expression_contracts.rs |
| cast_precedence | len/sizeのRust castが比較/negation/数値contextを変えない | compiler/tests/builtin_cast_precedence.rs |
| loop_rebinding | for変数の更新がiteration意味論を変えない | compiler/tests/codegen.rs |
| view_restore / handwritten Low | local ownerのviewを退役し引数borrowへ戻して返せる | compiler/tests/view_parameter_rebinding.rs |
| index_lookahead | 連続index speculative parseでnesting counterが漏れない | compiler/tests/frontend_contracts.rs |
| try_index_receiver | tryの結果をindexする括弧と所有権を保持する | compiler/tests/frontend_contracts.rs |
| string_bytes | control/Unicode/NUL/escapeとcrate::という文字列bytesを保持する | compiler/tests/literal_contracts.rs |
| nested_result | nested Resultのowned payloadと全match経路を保持する | compiler/tests/result_match.rs |
| temporary_view High/Low | 一時ownerのviewをlocalへ保存する時checkerが元の行で拒否する | compiler/tests/view_origins.rs |
| unsigned_negation High/Low | u64にsigned negationを適用せずcheckerが元の行で拒否する | compiler/tests/operator_types.rs |
| record_type_arguments High/Low | 未対応のrecord constructor型引数をparserが元の行で拒否する | compiler/tests/frontend_contracts.rs |
| shared_field_move High/Low | shared親の非Copy fieldをmoveしない | compiler/tests/shared_field_moves.rs |
| owned_result_discard / owned_result_return | owned Resultのdiscard拒否、対応するpass/returnは維持 | compiler/tests/result_discard.rs |
| static_absent_callback / static_local_escape | borrow無しreturn/function valueを保ちlocal ownerのview escapeは拒否 | compiler/tests/static_callback_views.rs, borrow_free_returns.rs |
| direct_view_read / handwritten Low | viewを自作read関数へ渡して読むpass | compiler/tests/ownership_calls.rs, ownership_boundaries.rs |
| borrowed_owner_move / handwritten Low | viewが後続で生存する元ownerを自作consumeへmoveするfail。High6行/Low7行 | compiler/tests/ownership_calls.rs, view_origins.rs |
| container_restore / handwritten Low | owned List[view]をlocal ownerから入力borrowへ復元して返すpass | compiler/tests/view_container_rebinding.rs, view_flow_foundation.rs |
| container_local_escape / handwritten Low | local ownerのviewをListに保持して返すfail。両方4行 | compiler/tests/view_container_rebinding.rs, view_origins.rs |

shared field明示copy/Arc move、実static str callback、extern owned Resultはruntime/adapterが必要なのでharnesses.jsonの実既存laneに接続する。std-onlyのstatic正例はNone/Optionのborrow-free returnとfunction valueに限定する。owned Result正例はNagi内でconstructorを追加せず、既存のpass/returnをRust側oracleから呼ぶ。

正例はstd-only native oracleを持ち、High直接Rustと保存Low経由Rustの両方を同じ期待値で照合する。handwritten LowもHigh emitterの結果をコピーせず別sourceとして保存する。negativeは診断意味、stage、source lineの3つを要求する。新しい表現を「checkerが受けたから」という理由で正例へ追加しない。

HTTP panicとSQL missing-columnは `harnesses.json` の実Cargo/HTTP/SQLite harnessが責任を持つ。登録確認だけでは保証しない。`python3 scripts/verify_compiler_contracts.py --run-linked` または同じ既存suiteの実行結果で検証する。
