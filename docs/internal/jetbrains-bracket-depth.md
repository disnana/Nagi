# JetBrains bracket depth colors

## 採用範囲

JetBrains pluginのNagi設定に、対応括弧を入れ子深度で色分けするopt-inを追加する。Highは`()`, `[]`、Lowは`()`, `[]`, `{}`を対象にする。開き括弧と対応する閉じ括弧は同じ色で、6色を超える深度はpaletteを循環する。

既定は無効。色は`Editor | Color Scheme | Nagi`の`Nested brackets`でthemeごとに上書きできる。global IDE設定や第三者pluginを変更しない。

## API判断

- [JetBrains Annotator](https://plugins.jetbrains.com/docs/intellij/annotator.html)は、`com.intellij.annotator`と`AnnotationHolder.textAttributes`によるcode色付けを公式に案内している。この公開extension pointだけを使用する。
- [Syntax Highlighter and Color Settings Page](https://plugins.jetbrains.com/docs/intellij/syntax-highlighter-and-color-settings-page.html)は、`TextAttributesKey`と`com.intellij.colorSettingsPage`によるtheme別の利用者設定を公式に案内している。各深度keyは既存のLanguage Defaultsへfallbackする。
- [Settings Tutorial](https://plugins.jetbrains.com/docs/intellij/settings-tutorial.html)に沿い、既存の`PersistentStateComponent`と`Configurable`へbooleanを追加する。

`RainbowVisitor`はsymbol名のsemantic rainbow用で、括弧深度の公開contractではない。private API、独自paint loop、editor markupの直接所有、IDEのglobal rainbow設定変更は使わない。

## 正しさと負荷

token-preserving lexerと同じ`NagiTokenScanner`を一度だけ線形走査する。string/comment token内の文字は括弧として扱わない。stack topと種類が一致したpairだけを色付けし、不一致ではstackを破棄する。未対応の開閉括弧は既存syntax highlighterの色を残す。長いfileでもtoken走査中に256 tokenごとにIDEのcancelを確認する。

1 fileあたり20,000 pairを上限とし、超えたfileでは追加色を全て省略する。部分色付けは行わない。Annotatorは`PsiFile`に対して1回だけspanを生成し、256 spanごとにcancelを確認する。

検証対象はmatching pair、混在する括弧種、string/comment、malformed/incomplete source、設定off/on、増分編集、20,000 pairの線形処理と上限、Color Settings Page登録、plugin verifierと4 IDE CI。実GUIでのtheme/accessibility受入は利用者確認が必要で、既定offと個別の色設定を維持する。
