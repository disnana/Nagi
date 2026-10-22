# JetBrains 0.1.4 公開準備

2026-10-22。利用者が#148の新しいActions候補について「動いた、、いいね0.1.4としてリリースかな」と回答した。対象は`9b678ec`のplugin source、run `39776573745`、artifact `12424564202`。内側ZIPはSHA-256 `2d9930e8f35600ddcb43c3731a010daf62ecf083fe60ea06880128b77c0ebae1`。この応答をホバー・色付け候補の受入として記録し、全GUI機能や全OSの検証へ拡張しない。

## 今回の差分

Gradleの版を0.1.3から0.1.4へ変更し、CHANGELOGと日英導入案内を更新する。その後の依頼に従い、IDEのDevelopment既定／Release明示選択と標準Run画面の引き渡し修正を追加する。compiler/runtime・ID・対応IDE下限・release workflowは変更しない。ホバーは[設計・検証記録](jetbrains-documentation-plan.md)、追加Run修正は[設計と検証](jetbrains-dev-run-plan.md)を参照する。

元sourceの最終243件、独立focused31件、IDEA/PyCharm stable/EAP全4経路、Verifier、共通ZIP、CodeQL、websiteは成功済み。新しい0.1.4のZIPは別成果物なので、最終HEADで四IDE test/Verifier・配布identity/checksum・必須CIを再確認する。旧候補ZIPを改名・再梱包して正式配布しない。

## 公開経路

#148の全採用範囲と最終版CI・独立確認、追加Run修正の必要なGUI確認が完了したらReadyとする。先行候補の受入は新しいRun修正のGUI検証を兼ねない。merge・tag操作は行わず、利用者のmerge後に既存main CIのJetBrains専用version判定と公開gateでGitHub Releaseを作る。公開されたとの報告にはtag・release・asset・checksumの読み戻しが必要。

GitHub Releaseの予定tagは`jetbrains-v0.1.4`、assetはIDEA/PyCharm共通`nagi-jetbrains-0.1.4.zip`と`.sha256`。Nagi本体とVS Codeの版は変更しない。Marketplace提出は従来どおり所有者の別操作で、今回agentから送信しない。Marketplace承認をGitHub公開から推測しない。

## 維持する制約

利用者から、Windows PyCharmで標準Runボタン／Shift+F10の実行時にRunコンソールだけが緑一色になり、タブを閉じると回復するとの追加報告がある。提供ログには破棄済みRun descriptor／UIへの参照もある。SDKの成功callback中に旧environmentが破棄されると、新しい受理済みdescriptorまでNagiが破棄する経路を再現し、所有権の移譲と取消を分離して修正した。プロジェクト終了時のprocess停止は維持する。緑の描画まで同じ原因かはWindowsの新候補で未確認であり、設定の強制変更や再利用無効化は行わない。Nagi本体0.2.1の公開gateとは分ける。個人情報を含む提供ログはcommitしない。

公開Nagi 0.2.0と連携し、既知の未対応compilerでは説明付きでReleaseを使う。Developmentは能力確認済みcompilerで利用し、標準Run設定でReleaseも選べる。CLIの既定Releaseは変えず、初回Rust依存のコンパイルは必要。新しいbare module import文法は公開0.2.0にはなく、相対path文字列の既存importを使用する。署名factsがないbuiltin `print`の説明は生成しない。IDEのhover設定、trusted project、現在snapshot、有限待機と取消を維持する。
