# JetBrains 0.1.3 candidate GUI trial / JetBrains 0.1.3候補GUI試用

## 日本語

### 実施状況

これはWindows上のIntelliJ IDEAとPyCharmで行う手動試用手順です。この作業では実際のGUI試用をしていません。結果はPASS / FAIL / NOT RUNで記録してください。

### 必須artifactと環境

- 試すPRの**同一commit SHA**から作られた共通JetBrains plugin ZIP。IDE検証後のActions artifact `release-jetbrains`を使います（正式なGitHub Releaseではありません）。
- 同じcommit SHAでWindows向けにbuildしたmatching `nagic.exe` のCI ZIP。compiler変更PRではActions artifact `release-windows-x86_64`を使います。ZIP内`release.json`の`commit`がplugin候補のsource commitと一致することを確認してください。公開済みcompilerや別commitのcompilerで代用しないでください。assist protocolが一致せず、semantic試用の結果になりません。
- Windows上のIntelliJ IDEAとPyCharm。候補pluginが対象とするIDE buildを記録します。IDEごとに別々に起動して試します。
- PR source SHA、Actions run ID、IDE名/build、plugin ZIP名とSHA-256、compiler ZIP名とSHA-256、compiler ZIP内`release.json`の`commit`を記録します。

現行CIはcompiler変更PRで共通plugin ZIPとWindows compiler ZIPを同じActions runのartifactとして生成します。pluginはIDE検証後の`release-jetbrains`、Windows compilerは`release-windows-x86_64`を使います。各`.sha256`を検証し、compiler ZIPを展開して`release.json`の`commit`がplugin候補に使ったPR source commitと一致することを確認してください。どちらかのartifactがない、checksumが失敗する、またはcommitが一致しない場合はsemantic試用を開始せずNOT RUNとしてください。公開済みの`nagic`を使った結果を候補の検証として記録しないでください。

### 試用project

同一projectに、manifest、High、Low、import先を用意します。たとえば次のファイルを使います。

```toml
# nagi.toml
entry = 'main.nagi'
native = ['sample.low']
```

```nagi
# main.nagi
import "library.nagi" as library
from std.ownership import move

def same_name() -> i64:
    return 1

def inspect():
    same_name = "local"
    gone = move(same_name)
    sam

def scope_test():
    if True:
        nested = 1
    nest

def main():
    print(library.answer())
```

```nagi
# library.nagi
def answer() -> i64:
    return 42
```

```low
# sample.low
fn low_value() -> i64 { return 7; }
```

試用時は`main.nagi`と`sample.low`をIDEで開きます。`native`設定によりLow fileをproject graphへ含めます。補完のshadow/move確認では、`sam`の位置にcaretを置き、moved localが候補として復活せず、同名globalへfallbackしないことを確認します。別のHigh/Low関数内でも同じ補完操作を試します。

### 手順

1. 共通plugin ZIPをIDEの **Settings → Plugins → ⚙ → Install Plugin from Disk** から入れ、IDEを再起動します。SettingsのNagi compiler pathを、必須artifactのmatching `nagic.exe` に設定します。plugin/compilerのcommit SHAが一致することを確認します。
2. まずIDEのproject trust状態を確認します。未信頼のprojectではsemantic assistanceが候補や古いdiagnosticを出さず、compilerを起動しないことを確認してから、明示的にprojectをtrustし、以降の項目を試します。標準Run Configurationも未信頼projectではwarningを表示して起動しないことを確認します。semantic assistanceにも起動を拒否した理由がfile-level noticeで表示されることを確認します。
3. HighとLowの両方を開きます。syntax highlightingと、IDEのBasic completion（必要ならCtrl+Space）を確認します。候補はcompilerがその位置で返したものだけで、自動挿入されないことを確認します。
4. `main.nagi`を変更して保存せず、型の異なるlocalを追加してその名前のprefixを入力します。補完の候補・型表示が未保存bufferに追随すること、ディスク上のファイルがassistのために保存されないことを確認します。Lowでも未保存の変更を使って同じ操作をします。
5. `nagi.toml`を変更して未保存のままsemantic assistanceを要求し、compilerが起動せず、新しい結果が適用されないことを確認します。manifest保存を促すfile-level noticeが表示されることを確認します。manifestを保存するとassistが再開することも確認します。
6. `same_name`を影で隠すlocalがmove後にある状態で補完します。`sam`の位置でmoved localが候補として復活せず、同名globalへ誤ってfallbackしないことを確認します。`scope_test`の`nest`では内側scopeのlocalが外側の候補に現れないことを確認します。
7. `library.`の後にcompletionを要求し、project importの公開名が候補になることを確認します。`library.answer()`の`answer`でGo to Definition（Ctrl+B / Ctrl+Click）を実行し、`library.nagi`の宣言へ移動することを確認します。compilerが正確なsource targetを返さない参照（現状のrecord field宣言など）は移動できない制約を記録します。
8. `library.nagi`に型エラーを作り、IDEにcompiler diagnosticが表示されることを確認します。未保存修正後に古いdiagnosticが残らず、import先sourceの行位置が合うことを確認します。Run consoleのcompiler source locationもクリックし、該当file/位置を開くことを確認します。
9. assistanceが処理中にHighまたはLow bufferを編集するか、そのfileを閉じて、古いsnapshotのcompletion/diagnosticが適用されないことを確認します。処理が速く目視できない場合は再試行し、見えなかった場合はPASS扱いにせず「cancellation not observed」と記録します。
10. 2つのtrusted projectを同時に開き、両方に同名のsource fileと異なるlocal名を用意します。project Aでcompletionを要求してからBへ切り替え、Aの候補・diagnosticがBへ混ざらないことを確認します。
11. `sam` / `nest`の未完入力と意図的に作ったtype errorを直し、Run前にprojectを有効な状態へ戻します。Run Configuration dropdownに`Nagi: main.nagi`があることを確認し、その設定を選択します。IDE上部左の標準Runボタンで起動し、Run windowに出力が表示されることを確認します。Stopボタンで停止し、compilerと子processが終了することを確認します。別途、`while True:`のように停止が必要なfixtureを使い、Stopが実行中processを止められることを確認します。Lowの実行確認にはmanifest project外のstandalone file（例：`fn main() -> unit { print(7); }`）を使い、Low source用設定を作って同じRun/Stopを行います。

標準Run Configurationも、trust済みprojectでのみ実行し、起動前に開いたfileが保存されることを確認してください。近くに`nagi.toml`がある場合は、選択sourceでなくmanifestのentryが実行されます。

### 結果記録

IDEごとに各手順のPASS / FAIL / NOT RUN、再現手順、IDE build、commit SHA、artifact SHA-256、関連するIDE logを記録します。GUIで確認していない動作を自動IDE fixtureやCI testだけでPASSにしないでください。

## English

### Status

This is a manual trial procedure for the unreleased 0.1.3 candidate in IntelliJ IDEA and PyCharm on Windows. The GUI trial was not run during this work. Record every item as PASS, FAIL, or NOT RUN.

### Required artifacts and environment

- The common JetBrains plugin ZIP built from the **same PR commit SHA** being tried. Use the `release-jetbrains` Actions artifact after IDE verification; this is not a formal GitHub Release.
- The matching Windows compiler CI ZIP containing `nagic.exe`, built from that exact commit SHA. For a compiler-change PR, use the `release-windows-x86_64` Actions artifact and verify that its `release.json` `commit` equals the plugin candidate's source commit. Do not substitute a published compiler or one from another commit; its assist protocol will not match, so it cannot validate semantic assistance.
- Windows installations of IntelliJ IDEA and PyCharm. Record the IDE build supported by the candidate and test each IDE in a separate session.
- Record the PR source SHA, Actions run ID, IDE and build, plugin ZIP name and SHA-256, compiler ZIP name and SHA-256, and the compiler ZIP's `release.json` `commit`.

For a compiler-change PR, current CI produces the common plugin ZIP and matching Windows compiler ZIP as artifacts in the same Actions run. Use `release-jetbrains` after IDE verification and `release-windows-x86_64` for the compiler. Verify both `.sha256` files, extract the compiler ZIP, and confirm its `release.json` `commit` matches the plugin candidate's PR source commit. If either artifact is missing, a checksum fails, or the commit differs, do not start the semantic trial; mark it NOT RUN. A result using a published `nagic` does not validate the candidate.

### Trial project

Use a project containing a manifest, High, Low, and an imported source. For example:

```toml
# nagi.toml
entry = 'main.nagi'
native = ['sample.low']
```

```nagi
# main.nagi
import "library.nagi" as library
from std.ownership import move

def same_name() -> i64:
    return 1

def inspect():
    same_name = "local"
    gone = move(same_name)
    sam

def scope_test():
    if True:
        nested = 1
    nest

def main():
    print(library.answer())
```

```nagi
# library.nagi
def answer() -> i64:
    return 42
```

```low
# sample.low
fn low_value() -> i64 { return 7; }
```

Open `main.nagi` and `sample.low` in the IDE. The `native` setting puts the Low file in the project graph. For the shadow/move completion check, place the caret after `sam` and confirm `same_name` does not return as either the moved local or a same-named global. Repeat completion in another High and Low function.

### Procedure

1. Install the common plugin ZIP from **Settings → Plugins → ⚙ → Install Plugin from Disk**, then restart the IDE. Set the Nagi compiler path to the matching `nagic.exe` from the required artifact. Confirm the plugin and compiler commit SHAs match.
2. Check the project's trust state. Verify that semantic assistance shows neither candidates nor stale diagnostics and does not launch the compiler in an untrusted project. Also confirm that the standard Run Configuration warns and refuses to start. Explicitly trust the project, then continue with the remaining checks. Verify that semantic assistance also displays a file-level notice explaining why it is unavailable.
3. Open both High and Low files. Check syntax highlighting and IDE Basic completion (use Ctrl+Space if needed). Confirm that suggestions come only from the compiler for that position and are not inserted automatically.
4. Edit `main.nagi` without saving, add a local with a different type, and type its prefix. Confirm completion and displayed type reflect the unsaved buffer and that assistance did not save the file to disk. Repeat with an unsaved change in Low.
5. Edit `nagi.toml` without saving and request semantic assistance. Confirm the compiler is not launched and no new result is applied. Verify the file-level notice asks you to save the manifest. Save the manifest and confirm assistance resumes.
6. Request completion after the local shadowing `same_name` has been moved. At the `sam` prefix, confirm the moved local does not return as a candidate and does not fall back to the same-named global. In `scope_test`, request completion at `nest` and confirm the inner-scope local is absent outside its scope.
7. Request completion after `library.` and confirm that the project import's exported names appear. Use Go to Definition (Ctrl+B / Ctrl+Click) on `answer` in `library.answer()` and confirm it opens the declaration in `library.nagi`. Record the limitation that references without an exact source target (including record-field declarations in the current response) cannot be navigated.
8. Add a type error to `library.nagi` and confirm the compiler diagnostic appears in the IDE. After an unsaved correction, confirm the old diagnostic disappears and the imported source location is correct. Click a compiler source location in the Run console and confirm it opens the expected file and position.
9. While assistance is running, edit a High or Low buffer or close its file. Confirm that completion/diagnostics from the old snapshot are not applied. Retry if the compiler finishes too quickly to observe; if cancellation is not observed, record that result instead of marking it PASS.
10. Open two trusted projects at once, each with the same source filename but different local names. Request completion in project A, switch to B, and confirm A's candidates and diagnostics do not appear in B.
11. Fix the incomplete `sam` / `nest` prefixes and any intentional type error, returning the project to a valid state before running. Confirm `Nagi: main.nagi` is available in the Run Configuration dropdown and select it. Start it with the standard Run button in the IDE's upper-left toolbar; confirm output appears in the Run window. Stop it with the standard Stop button and confirm the compiler and child process exit. Repeat with a fixture that needs stopping, such as `while True:`, to verify Stop terminates a running program. To test Low execution, use a standalone file outside the manifest project (for example, `fn main() -> unit { print(7); }`), create its Low Run Configuration, and repeat Run/Stop.

Also confirm that a standard Run Configuration requires a trusted project and saves open files before starting. When a nearby `nagi.toml` exists, it runs the manifest entry rather than the selected source file.

### Record results

For each IDE, record PASS / FAIL / NOT RUN per item, reproduction steps, IDE build, commit SHA, artifact SHA-256 values, and relevant IDE logs. Do not mark behavior observed only by an IDE fixture or CI test as a GUI PASS.
