package com.disnana.nagi;

import com.intellij.openapi.project.Project;
import com.intellij.openapi.vfs.VirtualFile;
import com.intellij.psi.codeStyle.CodeStyleSettings;
import com.intellij.psi.codeStyle.CommonCodeStyleSettings;
import com.intellij.psi.codeStyle.FileIndentOptionsProvider;
import org.jetbrains.annotations.NotNull;

/** Adapts eligible file indent options to spaces while preserving their configured widths. */
public final class NagiIndentOptionsProvider extends FileIndentOptionsProvider {
    private static final ThreadLocal<Boolean> RESOLVING_BASE = new ThreadLocal<>();

    @Override public boolean useOnFullReformat() { return true; }

    @Override public CommonCodeStyleSettings.IndentOptions getIndentOptions(@NotNull Project project,
                                                                            @NotNull CodeStyleSettings settings,
                                                                            @NotNull VirtualFile file) {
        if (file.getFileType() != NagiFileType.HIGH && file.getFileType() != NagiFileType.LOW) return null;
        if (Boolean.TRUE.equals(RESOLVING_BASE.get())) return spaceOnly(languageOptions(settings, file));
        RESOLVING_BASE.set(true);
        try {
            // A per-file settings modifier may bypass provider dispatch in getIndentOptionsByFile.
            // Visit each remaining eligible provider once, in the same order as the IDE lookup.
            for (var provider : FileIndentOptionsProvider.EP_NAME.getExtensionList()) {
                if (provider instanceof NagiIndentOptionsProvider || !provider.isAllowed(true)) continue;
                var options = provider.getIndentOptions(project, settings, file);
                if (options != null) return spaceOnly(options);
            }
            return spaceOnly(languageOptions(settings, file));
        } finally {
            RESOLVING_BASE.remove();
        }
    }

    private static CommonCodeStyleSettings.IndentOptions languageOptions(CodeStyleSettings settings, VirtualFile file) {
        return settings.getLanguageIndentOptions(file.getFileType() == NagiFileType.LOW
                ? NagiLanguage.LOW : NagiLanguage.HIGH);
    }

    private static CommonCodeStyleSettings.IndentOptions spaceOnly(CommonCodeStyleSettings.IndentOptions original) {
        var options = (CommonCodeStyleSettings.IndentOptions) original.clone();
        options.USE_TAB_CHARACTER = false;
        options.SMART_TABS = false;
        return options;
    }
}
