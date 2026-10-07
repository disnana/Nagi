# Nagi 0.1.10から0.1.11への移行

この案内はNagi 0.1.11への移行を説明します。0.1.11の入手可否は公式Release記録で確認してください。リポジトリ上の変更だけでは公開済みかどうかを判断できません。

## 既存の所有値を別のローカルへ代入する

所有する非Copy値を既存のローカルから別のローカルへ渡す代入には、明示的な`move`が必要になります。

変更前:

```nagi
destination = source
```

変更後:

```nagi
from std.ownership import move

destination = move(source)
```

既存値の裸の右辺代入は、型注釈付き/なしの宣言、再代入、括弧で囲んだ右辺でも拒否されます。この変更は、既存の所有ローカルから別のローカルへの代入に限られます。新しい値の生成やCopy値の代入は通常どおりです。引数、戻り値、field/index、try、matchの既存consume規則も変わらないため、すべての引数やreturnに`move`を追加する必要はありません。

移動後の`source`は再利用できず、借用中の所有者を移動することもできません。borrowed/shared親から非Copy fieldを取り出すことも拒否されます。読むには`view(source)`、独立した複製には`copy(view(source))`、安全に共有する場合は`share`と`clone_shared`を選びます。compilerは暗黙のcloneやshared化を挿入しません。標準`move`はcloneもallocationもしません。`std.ownership`からimportしていない同名のユーザー関数は通常の関数です。HighとLowには同じ規則が適用されます。

## Task結果handleとSupervisorの移行

新しいspawn bindingはscope内に結果handleを作ります。`await task`はhandleを一度消費し、子の実終了後に`Result[T, TaskFailure]`を返します。子が`Result[T, E]`を返すなら、受取型は`Result[Result[T, E], TaskFailure]`です。内側の業務ErrはTaskFailureではなく、これだけでは兄弟を取り消しません。

Supervisor monitorを型付きTaskへ移すときは、内側の業務Errを既存の親bodyのResult/try経路へ明示的に渡します。

変更前（legacy statement spawn）:

```nagi
async with scope:
    spawn monitor(group)
    spawn serve_web(...)
```

変更後（monitorのinner Resultを親へ渡す）:

```nagi
from std.task import message

async with scope:
    monitor_task = spawn monitor(group)
    spawn serve_web(...)
    received = await monitor_task
    match received:
        case Ok(inner):
            try inner
        case Err(failure):
            print(message(failure))
```

`try inner`によってSupervisorのterminal Errが親bodyのErrになります。これによりscopeは子への取消要求と実joinへ進みます。競合時に、先に観測したHTTP faultよりmonitor Errorが常に優先される保証はありません。外側のTaskFailureは`std.task.kind`でCopyな`TaskFailureKind`として、`std.task.message`でfailureに結び付いた`view[str]`として調べられます。TaskFailureからErrorへの暗黙変換はありません。表示・処理してもscope faultは消えません。

既存のstatement形式`spawn work()`は維持されます。このlegacy形式で`Result[unit, Error]`を返す子のErrは従来どおりscope faultです。Supervisor/HTTPの旧監視処理をbinding形式へ機械的に置換しないでください。旧HTTP spawnはfail-on-Err動作を維持します。

`discard(task)`はunitを返して受取だけを放棄します。Taskを停止・detachせず、faultを抑制せず、実joinやresource closeを省略しません。ただし、内側の業務Err自体はscope faultではないため、Errを返すmonitorをawaitせずdiscardすると、そのErrは親へ伝わりません。実証例ではHTTPが503を返し続け、独立したstop後にscopeが正常終了します。監視ErrでHTTPを停止させる移行ではdiscardではなく、await後に内側のErrを親へ伝えてください。

Taskは非Copy・非Clone・非sharedで、別scope、引数、return、field、container、wrapper、別Taskへescapeできません。moveはhandleと受取義務を一緒に移します。正常なbinding/scope出口とloop継続までにawaitまたはdiscardが必要です。短絡and/orの右辺やlazy env fallback内だけの受取では、省略経路に義務が残るため拒否されます。Taskを先に受け取り、得たResultを条件付きで使ってください。個別のTask close/cancel操作はありません。親Futureの同期Drop/unwindは取消要求までで、実join完了を待てません。yieldしない処理の強制停止、外部副作用のrollback、任意のRust Drop/panic payloadからの普遍的回復も保証されません。例と詳細は[Taskガイド](task-handles.md)と[Task結果サンプル](../test-nagi-code/library-examples/task-results/README.md)、[Supervised serviceサンプル](../test-nagi-code/library-examples/supervised-service/README.md)を参照してください。

## Rustからのcompiler embedding

Rust emitterの入力は`&Program`から`&checked::CheckedProgram`へ変わります。最終Low AST、native fragment、source provenanceを`check::finalize(primary, native, provenance)`へ渡し、その結果からRustを生成してください。`emit::rust_with_lines`も同じchecked inputを受け取ります。

物理fileを持たない手書きLowの最小例:

```rust
use nagic::{ast::Program, check, emit, parser, source::SourceProvenance};

fn main() {
    let primary = parser::parse("fn main() { print(42); }\n", false).unwrap();
    let checked = check::finalize(
        primary,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    let generated = emit::rust(&checked).unwrap();
    assert!(generated.contains("fn main"));
}
```

`CheckedProgram`のfieldsとconstructorはprivateで、`.program()`は読み取り専用です。ASTを変更した場合は、その変更後にfinalizeし直します。`check::check`や`integrate`だけでは、Rust emitterへ渡すchecked valueになりません。file入力はloaderから取得した`Sources::provenance()`を使い、High入力は通常のHigh→Low再parse→最終check経路を保ちます。Highのoriginを`user_low_unmapped()`として扱わないでください。

この例はemitterのRust文字列を確認するものです。生成Rustのnative buildを証明するものではありません。Rust adapterのcrate APIやtrait、最終Send/Sync/Clone制約、link/target/依存環境は引き続きrustcが検査します。

## ASTを直接扱うRust callerと網羅match

`ast::Stmt`にはprivateなchecker factsが加わり、外部Rust crateからstruct literalを構築できません。`parser::parse`または`source::load`でASTを取得し、必要な変換後にfinalizeしてください。`ast::S`には新しい`SpawnBind` variantが加わり、`stdlib::StandardModule`、`stdlib::Resource`、`stdlib::Operation`にもOwnership/Task/Auth関連variantが追加されます。これらを網羅matchするcallerは新しいvariantを処理する必要があります。

## build scriptの実行ファイル探索

実行ファイルは`build`または`run`が標準エラーに出す`native:`行から取得してください。成功したbuildごとに生成先の`.nagi/`内へ実行ファイルを保存するため、再buildで起動中の旧実行ファイルを上書きしません。内部filename、app ID、generation pathを固定値から組み立てないでください。

`NAGI_NATIVE_TARGET_DIR`は共有依存cacheを選ぶ変数で、成功世代の実行ファイルpathではありません。実行ファイルをcacheやCargo package名から探していたscriptは`native:`のpathを使ってください。詳細は[Project guide](projects.md)を参照してください。

## その他の受理・診断変更

typed constant検査は、compound式や対応するscalar aliasから確定できるゼロ除数、符号付きMINの-1による除算/剰余を拒否します。到達不能branch内の確定constantも検査されます。任意の関数/extern/field/indexの結果や、profile依存overflow値を確定constantとして伝播するものではありません。

元位置に対応付けられるRust errorは簡潔な表示が既定です。生成Rustのnote/suggestionも含める場合は`build/run --rust-diagnostics`を指定します。native、dependency、unmapped errorは省略されません。詳しくは[error handling guide](error-handling.md)を参照してください。

`std.auth.Principal`と`Grant[P]`はexperimental APIです。trusted Rust adapterが認証後にproofを作り、Nagiからの手動構築、JSON復元、copy、shared化はできません。認証サンプルの固定credentialは実運用の認証器ではありません。公開SQLite Pool/Transactionやshared actor messageは、このTask契約の一部ではありません。
