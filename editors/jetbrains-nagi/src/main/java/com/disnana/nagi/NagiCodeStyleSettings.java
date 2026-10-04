package com.disnana.nagi;

import com.intellij.psi.codeStyle.LanguageCodeStyleSettingsProvider;
import com.intellij.psi.codeStyle.CommonCodeStyleSettings;

public class NagiCodeStyleSettings extends LanguageCodeStyleSettingsProvider {
    private final boolean low;
    public NagiCodeStyleSettings() { this(false); }
    protected NagiCodeStyleSettings(boolean low) { this.low = low; }
    @Override public NagiLanguage getLanguage() { return low ? NagiLanguage.LOW : NagiLanguage.HIGH; }
    @Override public String getCodeSample(SettingsType type) {
        return low ? "fn main() {\n    print(42);\n}\n" : "def main():\n    print(42)\n";
    }
    @Override protected void customizeDefaults(CommonCodeStyleSettings common, CommonCodeStyleSettings.IndentOptions options) {
        options.INDENT_SIZE = 4;
        options.CONTINUATION_INDENT_SIZE = 4;
        options.TAB_SIZE = 4;
        options.USE_TAB_CHARACTER = false;
    }
    public static final class Low extends NagiCodeStyleSettings { public Low() { super(true); } }
}
