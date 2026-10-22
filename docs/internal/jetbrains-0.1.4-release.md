# JetBrains 0.1.4 公開準備

2026-10-22。利用者が#148の新しいActions候補について「動いた、、いいね0.1.4としてリリースかな」と回答した。対象は`9b678ec`のplugin source、run `39776573745`、artifact `12424564202`。内側ZIPはSHA-256 `2d9930e8f35600ddcb43c3731a010daf62ecf083fe60ea06880128b77c0ebae1`。この応答をホバー・色付け候補の受入として記録し、全GUI機能や全OSの検証へ拡張しない。

## 今回の差分

Gradleの版を0.1.3から0.1.4へ変更し、CHANGELOGと日英導入案内を更新する。機能実装・tests・compiler/runtime・ID・対応IDE下限・release workflowは変更しない。確認済み機能は[設計・検証記録](jetbrains-documentation-plan.md)を参照する。

元sourceの最終243件、独立focused31件、IDEA/PyCharm stable/EAP全4経路、Verifier、共通ZIP、CodeQL、websiteは成功済み。新しい0.1.4のZIPは別成果物なので、最終HEADで四IDE test/Verifier・配布identity/checksum・必須CIを再確認する。旧候補ZIPを改名・再梱包して正式配布しない。

## 公開経路

#148の全採用範囲と最終版CI・独立確認が完了したらReadyとする。merge・tag操作は行わず、利用者のmerge後に既存main CIのJetBrains専用version判定と公開gateでGitHub Releaseを作る。公開されたとの報告にはtag・release・asset・checksumの読み戻しが必要。

GitHub Releaseの予定tagは`jetbrains-v0.1.4`、assetはIDEA/PyCharm共通`nagi-jetbrains-0.1.4.zip`と`.sha256`。Nagi本体とVS Codeの版は変更しない。Marketplace提出は従来どおり所有者の別操作で、今回agentから送信しない。Marketplace承認をGitHub公開から推測しない。

## 維持する制約

公開Nagi 0.2.0と連携する。新しいbare module import文法は公開0.2.0にはなく、相対path文字列の既存importを使用する。署名factsがないbuiltin `print`の説明は生成しない。IDEのhover設定、trusted project、現在snapshot、有限待機と取消を維持する。Runのrelease profileと初回Rustビルド時間は今回変更しない。
