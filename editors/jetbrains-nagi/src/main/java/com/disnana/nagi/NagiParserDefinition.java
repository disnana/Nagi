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
import com.intellij.psi.ContributedReferenceHost;
import com.intellij.psi.PsiReference;
import com.intellij.psi.PsiReferenceService;
import com.intellij.psi.tree.IElementType;
import com.intellij.psi.tree.IFileElementType;
import com.intellij.psi.tree.TokenSet;
import org.jetbrains.annotations.NotNull;

/** A token-preserving PSI file, without a second implementation of Nagi's type checker. */
public class NagiParserDefinition implements ParserDefinition {
    private final boolean low;
    private static final IFileElementType HIGH_FILE = new IFileElementType(NagiLanguage.HIGH);
    private static final IFileElementType LOW_FILE = new IFileElementType(NagiLanguage.LOW);
    private static final IElementType HIGH_REFERENCE = new IElementType("NAGI_REFERENCE", NagiLanguage.HIGH);
    private static final IElementType LOW_REFERENCE = new IElementType("NAGI_LOW_REFERENCE", NagiLanguage.LOW);
    public NagiParserDefinition() { this(false); }
    protected NagiParserDefinition(boolean low) { this.low = low; }
    @Override public @NotNull Lexer createLexer(Project project) { return new NagiLexer(low); }
    @Override public @NotNull PsiParser createParser(Project project) {
        return (root, builder) -> {
            var marker = builder.mark();
            while (!builder.eof()) {
                IElementType token = builder.getTokenType();
                boolean reference = token == NagiTokens.get(NagiTokenScanner.Kind.IDENTIFIER)
                        || token == NagiTokens.get(NagiTokenScanner.Kind.STRING)
                        || token == NagiTokens.get(NagiTokenScanner.Kind.TYPE)
                        || token == NagiTokens.get(NagiTokenScanner.Kind.KEYWORD);
                if (reference) {
                    var name = builder.mark();
                    builder.advanceLexer();
                    name.done(low ? LOW_REFERENCE : HIGH_REFERENCE);
                } else builder.advanceLexer();
            }
            marker.done(root);
            return builder.getTreeBuilt();
        };
    }
    @Override public @NotNull IFileElementType getFileNodeType() { return low ? LOW_FILE : HIGH_FILE; }
    @Override public @NotNull TokenSet getCommentTokens() { return TokenSet.create(NagiTokens.get(NagiTokenScanner.Kind.COMMENT)); }
    @Override public @NotNull TokenSet getStringLiteralElements() { return TokenSet.create(NagiTokens.get(NagiTokenScanner.Kind.STRING)); }
    @Override public @NotNull PsiElement createElement(ASTNode node) {
        return node.getElementType() == HIGH_REFERENCE || node.getElementType() == LOW_REFERENCE
                ? new ReferenceElement(node) : new ASTWrapperPsiElement(node);
    }
    /** Position host only. Its target always comes from the compiler snapshot. */
    static final class ReferenceElement extends ASTWrapperPsiElement implements ContributedReferenceHost {
        private ReferenceElement(ASTNode node) { super(node); }
        @Override public PsiReference @NotNull [] getReferences() {
            return PsiReferenceService.getService().getContributedReferences(this);
        }
        @Override public PsiReference getReference() {
            var references = getReferences();
            return references.length == 1 ? references[0] : null;
        }
    }
    @Override public @NotNull PsiFile createFile(@NotNull FileViewProvider provider) {
        boolean isLow = low;
        return new PsiFileBase(provider, low ? NagiLanguage.LOW : NagiLanguage.HIGH) {
            @Override public @NotNull NagiFileType getFileType() { return isLow ? NagiFileType.LOW : NagiFileType.HIGH; }
        };
    }
    public static final class Low extends NagiParserDefinition { public Low() { super(true); } }
}
