package com.disnana.nagi;

import com.intellij.lexer.Lexer;
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors;
import com.intellij.openapi.editor.HighlighterColors;
import com.intellij.openapi.editor.colors.TextAttributesKey;
import com.intellij.openapi.fileTypes.SyntaxHighlighter;
import com.intellij.openapi.fileTypes.SyntaxHighlighterBase;
import com.intellij.openapi.fileTypes.SyntaxHighlighterFactory;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.vfs.VirtualFile;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NotNull;

public final class NagiSyntaxHighlighter extends SyntaxHighlighterBase {
    private final boolean low;
    public NagiSyntaxHighlighter(boolean low) { this.low = low; }
    @Override public @NotNull Lexer getHighlightingLexer() { return new NagiLexer(low); }
    @Override public TextAttributesKey @NotNull [] getTokenHighlights(IElementType tokenType) {
        for (var kind : NagiTokenScanner.Kind.values()) {
            if (tokenType != NagiTokens.get(kind)) continue;
            var color = switch (kind) {
                case COMMENT -> DefaultLanguageHighlighterColors.LINE_COMMENT;
                case STRING -> DefaultLanguageHighlighterColors.STRING;
                case NUMBER -> DefaultLanguageHighlighterColors.NUMBER;
                case KEYWORD -> DefaultLanguageHighlighterColors.KEYWORD;
                case TYPE -> DefaultLanguageHighlighterColors.CLASS_NAME;
                case OPERATOR -> DefaultLanguageHighlighterColors.OPERATION_SIGN;
                case LPAREN, RPAREN -> DefaultLanguageHighlighterColors.PARENTHESES;
                case LBRACKET, RBRACKET -> DefaultLanguageHighlighterColors.BRACKETS;
                case LBRACE, RBRACE -> DefaultLanguageHighlighterColors.BRACES;
                case BAD -> HighlighterColors.BAD_CHARACTER;
                default -> null;
            };
            return color == null ? EMPTY : pack(color);
        }
        return EMPTY;
    }
    public static class Factory extends SyntaxHighlighterFactory {
        @Override public @NotNull SyntaxHighlighter getSyntaxHighlighter(Project project, VirtualFile file) {
            return new NagiSyntaxHighlighter(false);
        }
    }
    public static final class LowFactory extends Factory {
        @Override public @NotNull SyntaxHighlighter getSyntaxHighlighter(Project project, VirtualFile file) {
            return new NagiSyntaxHighlighter(true);
        }
    }
}
