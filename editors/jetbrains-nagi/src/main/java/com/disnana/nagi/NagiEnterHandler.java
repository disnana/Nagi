package com.disnana.nagi;

import com.intellij.codeInsight.editorActions.enter.EnterHandlerDelegateAdapter;
import com.intellij.openapi.actionSystem.DataContext;
import com.intellij.openapi.editor.Editor;
import com.intellij.psi.PsiFile;
import com.intellij.application.options.CodeStyle;
import org.jetbrains.annotations.NotNull;

public final class NagiEnterHandler extends EnterHandlerDelegateAdapter {
    @Override public Result postProcessEnter(@NotNull PsiFile file, @NotNull Editor editor, @NotNull DataContext context) {
        if (file.getLanguage() != NagiLanguage.HIGH && file.getLanguage() != NagiLanguage.LOW) return Result.Continue;
        var document = editor.getDocument();
        int caret = editor.getCaretModel().getOffset();
        int line = document.getLineNumber(caret);
        if (line == 0) return Result.Continue;
        int start = document.getLineStartOffset(line);
        String prefix = document.getText(new com.intellij.openapi.util.TextRange(start, caret));
        if (!prefix.isBlank()) return Result.Continue;
        var options = CodeStyle.getIndentOptions(file);
        String wanted = NagiStructure.indentAt(document.getCharsSequence().subSequence(0, document.getLineEndOffset(line - 1)),
                file.getLanguage() == NagiLanguage.LOW, options.INDENT_SIZE);
        if (!prefix.equals(wanted)) {
            document.replaceString(start, caret, wanted);
            editor.getCaretModel().moveToOffset(start + wanted.length());
        }
        return Result.Continue;
    }
}
