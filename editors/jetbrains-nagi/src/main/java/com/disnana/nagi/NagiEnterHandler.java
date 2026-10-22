package com.disnana.nagi;

import com.intellij.codeInsight.editorActions.enter.EnterHandlerDelegateAdapter;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.openapi.actionSystem.DataContext;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.editor.ex.DocumentEx;
import com.intellij.openapi.editor.actionSystem.EditorActionHandler;
import com.intellij.openapi.util.Key;
import com.intellij.openapi.util.Ref;
import com.intellij.psi.PsiFile;
import com.intellij.application.options.CodeStyle;
import com.intellij.openapi.util.TextRange;
import org.jetbrains.annotations.NotNull;

public final class NagiEnterHandler extends EnterHandlerDelegateAdapter {
    private record LastEnter(long stamp, int sequence, int caret) {}
    private static final Key<LastEnter> LAST_ENTER = Key.create("nagi.last.enter");
    private static final Key<Boolean> REPEATED_ENTER = Key.create("nagi.repeated.enter");

    @Override public Result preprocessEnter(@NotNull PsiFile file, @NotNull Editor editor,
                                            @NotNull Ref<Integer> caretOffset, @NotNull Ref<Integer> caretAdvance,
                                            @NotNull DataContext context, EditorActionHandler originalHandler) {
        LastEnter previous = editor.getUserData(LAST_ENTER);
        editor.putUserData(LAST_ENTER, null);
        editor.putUserData(REPEATED_ENTER, null);
        if (file.getLanguage() != NagiLanguage.HIGH
                || !CodeInsightSettings.getInstance().SMART_INDENT_ON_ENTER
                || editor.getCaretModel().getCaretCount() != 1
                || editor.getSelectionModel().hasSelection()) return Result.Continue;
        if (!(editor.getDocument() instanceof DocumentEx extended)
                || extended.getModificationSequence() < 0) return Result.Continue;
        // Only an unchanged, automatically indented line qualifies. Typing,
        // Backspace, paste and undo invalidate this editor-local observation.
        // DocumentEx defaults to a constant zero when no sequence is available.
        // A fresh real document may start at zero and advance during this Enter.
        editor.putUserData(REPEATED_ENTER, extended.getModificationSequence() > 0
                && previous != null
                && previous.stamp() == editor.getDocument().getModificationStamp()
                // Undo/redo may restore a stamp. The edit sequence also has to match.
                && previous.sequence() == extended.getModificationSequence()
                && previous.caret() == caretOffset.get());
        return Result.Continue;
    }

    @Override public Result postProcessEnter(@NotNull PsiFile file, @NotNull Editor editor, @NotNull DataContext context) {
        Boolean repeated = editor.getUserData(REPEATED_ENTER);
        editor.putUserData(REPEATED_ENTER, null);
        if (file.getLanguage() != NagiLanguage.HIGH && file.getLanguage() != NagiLanguage.LOW) return Result.Continue;
        if (!CodeInsightSettings.getInstance().SMART_INDENT_ON_ENTER || editor.getCaretModel().getCaretCount() != 1) return Result.Continue;
        var document = editor.getDocument();
        int caret = editor.getCaretModel().getOffset();
        int line = document.getLineNumber(caret);
        if (line == 0) return Result.Continue;
        int start = document.getLineStartOffset(line);
        if (caret - start > NagiEditorIndent.MAX_SCAN_CHARS) return Result.Continue;
        String prefix = document.getText(new TextRange(start, caret));
        if (!prefix.isBlank()) return Result.Continue;
        var options = CodeStyle.getIndentOptions(file);
        String wanted = NagiEditorIndent.afterLine(document, editor.getHighlighter(), line - 1,
                file.getLanguage() == NagiLanguage.LOW, options.INDENT_SIZE, options.CONTINUATION_INDENT_SIZE,
                options.TAB_SIZE, Boolean.TRUE.equals(repeated));
        if (wanted != null && !prefix.equals(wanted)) {
            document.replaceString(start, caret, wanted);
            editor.getCaretModel().moveToOffset(start + wanted.length());
        }
        int after = editor.getCaretModel().getOffset();
        if (file.getLanguage() == NagiLanguage.HIGH && repeated != null
                && after == document.getLineEndOffset(line) && document instanceof DocumentEx extended
                && extended.getModificationSequence() > 0) {
            editor.putUserData(LAST_ENTER, new LastEnter(document.getModificationStamp(),
                    extended.getModificationSequence(), after));
        }
        return Result.Continue;
    }
}
