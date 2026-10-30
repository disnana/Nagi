# NagiアプリのGitHub Actions

プロジェクトCI generatorを使い、アプリのworkflowを作成できます。Python 3.10以上が必要です。公開済み0.2.3には`tools/generate_project_ci.py`を同梱しています。0.2.3の配布物またはこのソースcheckoutを使ってください。公開0.2.0には同梱されていません。`nagi.toml`があるアプリのルートで実行します。

```bash
# 0.2.3配布物を展開したdirectoryを指定
python /path/to/nagi/tools/generate_project_ci.py \
  --nagi-version 0.2.3 --output .github/workflows/nagi.yml
```

ソースcheckoutを使う場合:

```bash
python /path/to/Nagi/scripts/generate_project_ci.py \
  --nagi-version 0.2.3 --output .github/workflows/nagi.yml
```

generatorはPython標準ライブラリだけを使います。指定先へworkflowを書き込み、既存fileは上書きせず、必要な親directoryを作成します。既存fileを置き換えるときだけ`--force`を指定してください。保存前に内容を確認する場合は、標準出力へ表示できます。

```bash
python /path/to/Nagi/scripts/generate_project_ci.py
```

現在のgeneratorはNagi `0.2.1`を既定値にするため、上の例では公開済みの現行版`0.2.3`を明示しています。generatorの既定値を、利用するNagiのversionだと決めつけないでください。native buildにはstable Rustを使います。生成時に設定を変更できます。

```bash
python /path/to/Nagi/scripts/generate_project_ci.py \
  --nagi-version 0.2.3 --toolchain stable \
  --output .github/workflows/nagi.yml
```

Nagiのversionは完全な`X.Y.Z`形式で指定します。buildは既定でrelease profileを使います。`--os-matrix`を付けるとUbuntu・macOS・Windowsでbuildします。省略時はUbuntuだけです。

公開済み0.2.3のgeneratorでは、開発用CI向けに`--build-profile dev`を選べます。生成先のNagi CLIは公開0.2.1以上が必要です。

```bash
python /path/to/Nagi/scripts/generate_project_ci.py \
  --nagi-version 0.2.3 --build-profile dev --os-matrix \
  --output .github/workflows/nagi.yml
```

指定したprofileはWindowsとUnixの両方で、生成したworkflowのすべてのイベントに適用します。devでも`nagic check`とbuild内の完全検査、Cargoの依存判定は維持します。devにはoverflow検査などreleaseとの動作差があるため、[開発用ビルド](development-builds.md)を確認してください。本番向けのrelease buildやアプリのtestをdev buildで置き換えないでください。release用のworkflowを別に設ける場合は、同じイベントで互いを取り消さないよう`concurrency.group`も分けます。

`--build-profile release`を明示すると既定と同じworkflowになります。Nagi 0.2.1以上では`nagic build --profile release|dev`を生成し、0.2.0を指定した場合は従来の`nagic build`を維持します。0.2.0とdevの組合せは、出力fileを書き換える前にエラーになります。

workflowはnative buildの前に、別jobで`nagic check`を実行します。このjobはRustをinstallせずCargoも使わないため、型やprojectのエラーをnative compileより先に確認できます。次のbuild jobでは公式installerを使って固定したNagiのreleaseをinstallし、指定したRust toolchainをinstallしてから、公開版CLIが対応する`nagic build`を実行します。step summaryにはcheckまたはbuildにかかった実測時間とexit codeを記録します。この時間はrunner上での実行時間であり、サンプルやbenchmarkの測定値ではありません。

Nagiに全アプリ共通の`test` commandはありません。buildの後へ、生成した実行fileをfixtureで起動する、既存のtest harnessを使う、生成APIの出力を検査するなど、アプリ固有のcheckを追加してください。生成fileの先頭に追加位置のコメントがあります。compilerのcheckやbuildをアプリのtestとして記載しないでください。

build jobはCargoのregistry・Git downloadと、`NAGI_NATIVE_TARGET_DIR`で指定したdirectoryをcacheします。完全一致するcache keyにはNagi version、build profile、OS、CPU architecture、Rust toolchain、Cargo lockfileとmanifest、`nagi.toml`が含まれ、広いfallback keyはありません。このソースcheckoutのgeneratorはprofile別の`v2` keyを使うため、従来の`v1` cacheは復元せず、移行後の初回はcacheを作り直します。forkからのpull requestを含め、pull requestは一致するcacheをrestoreできますが、cacheを保存するのは成功したdefault branchへのpushだけです。これにより、信頼できないpull request buildが、後から信頼済みbuildでrestoreするcacheを書き換えることを防ぎます。workflowのpermissionは`contents: read`のみで、checkout credentialの永続化を無効にし、secretを使わず、`pull_request_target`も使いません。

既定workflowはPython、`uv`、`sccache`をinstallしません。アプリ固有testが宣言済みのPython dependencyを使う場合は、Pythonのsetupと既存requirementsまたはlockfileからのinstallを追加してください。`uv`はprojectが既に使っている場合だけ追加します。`sccache`はnative buildの規模とcache運用に見合う場合に追加できる任意の調整項目です。動作に必須ではありません。

`nagi.toml`やcommandの動作、native target cacheについては[project guide](projects.md)を参照してください。

通常のpushはdefault branch（既定`main`）に限定し、作業branchはPRイベントで検査するため二重実行を避けます。default branchが異なる場合は生成時に`--default-branch master`などを指定してください。新しい同一PRの実行は古い実行を取り消します。全commitの処理が必要なrelease workflowではこの取消設定を別途調整してください。Rust actionはcommit SHAを固定し、toolchainはactionのinputで渡します。指定版のRustと依存crateの互換性は別途確認してください。

生成した`build/*/Cargo.lock`もnative targetと一緒にrestore/saveします。これにより、新しいrunnerでも前回の依存解決を引き継ぎます。既定以外の出力先を使う場合はlockfileのpathを調整してください。生成snapshot全体はcacheしません。cacheは依存更新のレビューやlockfileの管理を代替しません。

GitHubのbase/default branch cacheはfork PRから読み出せます。workflowの`if`でrestoreを省略しても機密保護の境界にはなりません。native成果物、lockfile、Cargo registry・Git downloadにも、ソース、private dependency、`env!`で埋め込んだ値が残り得ます。cacheする内容はforkへ公開可能なものに限定してください。機密情報を含む場合は該当cache pathを削除し、共有可能なものがなければcache stepを外すか、アクセス制御された別backendを使ってください。self-hosted runnerではPRごとの一時workspaceと権限を分離し、本番credentialを持つ常設runnerで信頼できないPRを実行しないでください。sccacheを追加する場合も読取権限と保存権限の両方を確認します。
