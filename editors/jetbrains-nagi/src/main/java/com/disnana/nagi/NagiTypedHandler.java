package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.editorActions.TypedHandlerDelegate;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.actionSystem.DataContext;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.util.Key;
import com.intellij.psi.PsiFile;
import org.jetbrains.annotations.NotNull;

/** Adds small, bounded indentation corrections after branch and closing-delimiter input. */
public final class NagiTypedHandler extends TypedHandlerDelegate {
    private static final Key<InputState> INPUT_STATE = Key.create("nagi.typed.input.state");
    private static final Key<Boolean> REPLACED_SELECTION = Key.create("nagi.typed.replaced.selection");

    @Override public void newTypingStarted(char character, @NotNull Editor editor, @NotNull DataContext context) {
        editor.putUserData(INPUT_STATE, null);
        editor.putUserData(REPLACED_SELECTION, null);
    }

    @Override public @NotNull Result beforeSelectionRemoved(char character, @NotNull Project project,
                                                            @NotNull Editor editor, @NotNull PsiFile file) {
        editor.putUserData(REPLACED_SELECTION, isRelevant(character, file)
                && editor.getSelectionModel().hasSelection());
        return Result.CONTINUE;
    }

    @Override
    public @NotNull Result beforeCharTyped(char character, @NotNull Project project, @NotNull Editor editor,
                                           @NotNull PsiFile file, @NotNull com.intellij.openapi.fileTypes.FileType fileType) {
        editor.putUserData(INPUT_STATE, null);
        boolean replaced = Boolean.TRUE.equals(editor.getUserData(REPLACED_SELECTION));
        editor.putUserData(REPLACED_SELECTION, null);
        if (!isRelevant(character, file) || !CodeInsightSettings.getInstance().SMART_INDENT_ON_ENTER) return Result.CONTINUE;
        boolean blocked = replaced || editor.getCaretModel().getCaretCount() != 1 || editor.getSelectionModel().hasSelection();
        editor.putUserData(INPUT_STATE, new InputState(character, blocked));

        // The platform's paired-character handler may overtype an existing closer without
        // invoking charTyped(). Align that same existing token before the overtype proceeds.
        if (!blocked && isCloser(character)) {
            int offset = editor.getCaretModel().getOffset();
            var document = editor.getDocument();
            if (offset < document.getTextLength() && document.getCharsSequence().charAt(offset) == character) {
                NagiTypedIndent.alignTypedCloser(file, editor, character, offset, CodeStyle.getIndentOptions(file));
            }
        }
        return Result.CONTINUE;
    }

    @Override
    public @NotNull Result charTyped(char character, @NotNull Project project, @NotNull Editor editor,
                                     @NotNull PsiFile file) {
        InputState state = editor.getUserData(INPUT_STATE);
        editor.putUserData(INPUT_STATE, null);
        if (!isRelevant(character, file) || !CodeInsightSettings.getInstance().SMART_INDENT_ON_ENTER
                || editor.getCaretModel().getCaretCount() != 1 || editor.getSelectionModel().hasSelection()
                || state == null || state.blocked() || state.character() != character) return Result.CONTINUE;

        var options = CodeStyle.getIndentOptions(file);
        if (character == ':') {
            int offset = editor.getCaretModel().getOffset() - 1;
            if (offset >= 0 && editor.getDocument().getCharsSequence().charAt(offset) == ':') {
                NagiTypedIndent.alignTypedColon(file, editor, offset, options);
            }
        } else if (isCloser(character)) {
            int offset = editor.getCaretModel().getOffset() - 1;
            if (offset >= 0 && editor.getDocument().getCharsSequence().charAt(offset) == character) {
                NagiTypedIndent.alignTypedCloser(file, editor, character, offset, options);
            }
        }
        return Result.CONTINUE;
    }

    private static boolean isRelevant(char character, PsiFile file) {
        // Ordinary typing must not resolve code style or allocate input state.
        return file.getLanguage() == NagiLanguage.HIGH && (character == ':' || character == ')' || character == ']')
                || file.getLanguage() == NagiLanguage.LOW && isCloser(character);
    }

    private static boolean isCloser(char character) {
        return character == ')' || character == ']' || character == '}';
    }

    private record InputState(char character, boolean blocked) {}
}
