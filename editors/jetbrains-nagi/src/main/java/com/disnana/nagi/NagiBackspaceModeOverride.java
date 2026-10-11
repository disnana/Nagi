package com.disnana.nagi;

import com.intellij.codeInsight.editorActions.BackspaceModeOverride;
import com.intellij.codeInsight.editorActions.SmartBackspaceMode;
import com.intellij.psi.PsiFile;
import org.jetbrains.annotations.NotNull;

/** Use the IDE's indent-width backspace action for Nagi's explicit whitespace blocks. */
public final class NagiBackspaceModeOverride extends BackspaceModeOverride {
    @Override public @NotNull SmartBackspaceMode getBackspaceMode(@NotNull PsiFile file, @NotNull SmartBackspaceMode selected) {
        // AUTOINDENT asks a formatter for a target. Nagi uses an explicit width
        // and a bounded Enter handler, so the IDE's INDENT mode is the right
        // standard action. Keep a user's explicit OFF or INDENT selection.
        return selected == SmartBackspaceMode.AUTOINDENT ? SmartBackspaceMode.INDENT : selected;
    }
}
