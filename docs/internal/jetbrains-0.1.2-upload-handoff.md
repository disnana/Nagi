# Nagi for JetBrains 0.1.2の提出用ZIP

この文書は提出用ビルドを準備した時点の履歴。後続のユーザー承認・マージ・GitHub公開は[0.1.2公開引継ぎ](handoffs/2026-10-09-jetbrains-0.1.2-release.md)と実際のPR/Releaseを正とする。

記録日: 2026-10-09 JST。ユーザー指示は「自分がアップロードするので0.1.2として発行」。JetBrainsプラグインの版更新とCI成果物の提供を実行する。Marketplaceアップロードはユーザーが行う。GUI検証完了まで#102をDraft・未mergeに保ち、tag/GitHub Release/自動Marketplace送信を行わない。

## 固定する入力と契約

- 基準は#102の検証済みhead `6c23761d0e09bf3c4ffdfc585d9940288df71313`。旧CI merge `9c6b4847f135d6d9f6ed094c46f01bc085c17369`と共通tree `b478a09ca0d38a2e0c9553be941123848dc7f4c1`を読戻し済み。
- 変更はGradleのplugin版0.1.1→0.1.2、CHANGELOG、日英README、提出/GUI検証の記録だけ。plugin source・ID `com.disnana.nagi`・IDE minimum・Java21 target/EAP javac25・compiler/runtimeを維持する。
- 配布ファイルはIDEA/PyCharm共通 `nagi-jetbrains-0.1.2.zip` と `.sha256`。candidateを一度生成し、四SDKすべてのtest/Verifierが成功した同じbytesだけを `release-jetbrains` artifactへ昇格する。
- IDEA最低build251.25410.109、PyCharm最低build251.25410.122（2025.1.1）。CI EAPの実buildは新runのmetadataで確認する。初期2025.1/2024.3は対象外。
- 公開済みMarketplace0.1.1は既存ページ34891。0.1.2が公開されたとは、ユーザーのアップロード/承認状況を読み戻すまで書かない。Getting StartedはユーザーのWeb UI管理を維持する。

## 成果物の取得と提出

PR #102の実際の0.1.2 headと最新Checksを読み戻し、成功runのArtifacts → `release-jetbrains` をダウンロードする。Actions外側ZIPを展開し、内側のplugin ZIPとSHA-256を照合する。内側ZIPは展開せず、Install Plugin from Diskまたは既存Marketplaceページへの新version提出に使う。新しいplugin ID/登録を作らない。ZIP内のplugin.xmlのversionが0.1.2、IDがcom.disnana.nagiであることをCIで検査する。

Marketplaceへはユーザーが同じ内側ZIPをアップロードする。IDEA/PyCharm共通で一つの提出とし、対応buildを確認する。提出/公開が行われても、それを#102のmerge承認と推測しない。

## Windows GUI検証と次PR

具体的なdownload link・独立IDE設定の手順・High/Low正負サンプル・チェックリストはPR #102本文を最新0.1.2 runへ更新する。手動ZIPなら設定を分けたIDEでinstallする。同じIDを通常profileへinstallすると既存版が置換される。自動更新による混在を避け、GUI結果はIDE build・run・head・版・checksumと一緒に記録する。

Windows x64のcompilerとruntimeは基準6c23761の `release-windows-x86_64` artifact（checks37809181474）を使える。今回compiler sourceは変更しない。Rust/Cargo MSVCとVisual Studio Build ToolsのC++/Windows SDKがbuildに必要。Compiler executableは展開したnagic.exeの絶対パスを指定し、正式インストーラー/current/User PATHは切り替えない。

High・保存Low・手書きLowの構文認識、Check/Run/Terminal build、gutter、元位置付きnegative診断、trust拒否、Stop、IDE操作の継続を確認する。自動型診断・型補完・定義ジャンプはこの版の機能としない。

次PRの補完・ナビゲーション・編集中診断も、実装/CI/独立review後に別のversionで識別できる共通ZIPと対応compiler、機能ごとのGUI項目を提供する。正式versionの選択とMarketplace送信は、その時点のユーザー承認に従う。

## 検証記録の読み方

旧headの四SDK各40/40・4 OS・website・独立Sol reviewは基準実装の証拠。今回0.1.2の成功へ置換しない。0.1.2 headの必須CI、四SDK各40/40・skip0、Verifier、内側version/ID/checksumと元candidateの昇格を別に確認する。自己SHAはこの本文へ埋めず、PR本文と実HEAD/Checksを正とする。GUI手順はユーザー実施待ちで、CI/headless実行を手動GUI成功と呼ばない。

版変更直後のCI guardは旧0.1.1固定期待だけでRED（63件中1件失敗）。明示された版更新へ期待を0.1.2に同期する。ID・minimum・Java target・四SDK gate・trust負例・警告muteの検査は維持する。原ログは環境内 `/workspace/nagi-jetbrains-012-validation/version-guard-red.log` に保存する。
