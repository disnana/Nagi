package com.disnana.nagi;

import com.intellij.lang.ASTNode;
import com.intellij.lang.ParserDefinition;
import com.intellij.lang.PsiParser;
import com.intellij.extapi.psi.ASTWrapperPsiElement;
import com.intellij.extapi.psi.PsiFileBase;
import com.intellij.lexer.Lexer;
import com.intellij.openapi.project.Project;
import com.intellij.psi.FileViewProvider;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.tree.IFileElementType;
import com.intellij.psi.tree.TokenSet;
import org.jetbrains.annotations.NotNull;

/** A token-preserving PSI file, without a second implementation of Nagi's type checker. */
public class NagiParserDefinition implements ParserDefinition {
    private final boolean low;
    private static final IFileElementType HIGH_FILE = new IFileElementType(NagiLanguage.HIGH);
    private static final IFileElementType LOW_FILE = new IFileElementType(NagiLanguage.LOW);
    public NagiParserDefinition() { this(false); }
    protected NagiParserDefinition(boolean low) { this.low = low; }
    @Override public @NotNull Lexer createLexer(Project project) { return new NagiLexer(low); }
    @Override public @NotNull PsiParser createParser(Project project) {
        return (root, builder) -> {
            var marker = builder.mark();
            while (!builder.eof()) builder.advanceLexer();
            marker.done(root);
            return builder.getTreeBuilt();
        };
    }
    @Override public @NotNull IFileElementType getFileNodeType() { return low ? LOW_FILE : HIGH_FILE; }
    @Override public @NotNull TokenSet getCommentTokens() { return TokenSet.create(NagiTokens.get(NagiTokenScanner.Kind.COMMENT)); }
    @Override public @NotNull TokenSet getStringLiteralElements() { return TokenSet.create(NagiTokens.get(NagiTokenScanner.Kind.STRING)); }
    @Override public @NotNull PsiElement createElement(ASTNode node) { return new ASTWrapperPsiElement(node); }
    @Override public @NotNull PsiFile createFile(@NotNull FileViewProvider provider) {
        boolean isLow = low;
        return new PsiFileBase(provider, low ? NagiLanguage.LOW : NagiLanguage.HIGH) {
            @Override public @NotNull NagiFileType getFileType() { return isLow ? NagiFileType.LOW : NagiFileType.HIGH; }
        };
    }
    public static final class Low extends NagiParserDefinition { public Low() { super(true); } }
}
