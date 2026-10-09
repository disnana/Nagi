package com.disnana.nagi;

import com.intellij.lang.ASTFactory;
import com.intellij.psi.ContributedReferenceHost;
import com.intellij.psi.PsiReference;
import com.intellij.psi.PsiReferenceService;
import com.intellij.psi.impl.source.tree.LeafElement;
import com.intellij.psi.impl.source.tree.LeafPsiElement;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** Makes exact identifier/import tokens participate in the IDE reference framework. */
public final class NagiASTFactory extends ASTFactory {
    @Override public @Nullable LeafElement createLeaf(@NotNull IElementType type, @NotNull CharSequence text) {
        if (type == NagiTokens.get(NagiTokenScanner.Kind.IDENTIFIER)
                || type == NagiTokens.get(NagiTokenScanner.Kind.STRING)) return new ReferenceLeaf(type, text);
        return null;
    }

    private static final class ReferenceLeaf extends LeafPsiElement implements ContributedReferenceHost {
        private ReferenceLeaf(IElementType type, CharSequence text) { super(type, text); }
        @Override public PsiReference @NotNull [] getReferences() {
            return PsiReferenceService.getService().getContributedReferences(this);
        }
        @Override public @Nullable PsiReference getReference() {
            var references = getReferences();
            return references.length == 1 ? references[0] : null;
        }
    }
}
