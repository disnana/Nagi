package com.disnana.nagi;

import com.intellij.lang.ASTNode;
import com.intellij.lang.folding.FoldingBuilderEx;
import com.intellij.lang.folding.FoldingDescriptor;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.util.TextRange;
import com.intellij.psi.PsiElement;
import org.jetbrains.annotations.NotNull;

public final class NagiFoldingBuilder extends FoldingBuilderEx {
    @Override public FoldingDescriptor @NotNull [] buildFoldRegions(@NotNull PsiElement root, @NotNull Document document, boolean quick) {
        boolean low = root.getLanguage() == NagiLanguage.LOW;
        return NagiStructure.folds(document.getCharsSequence(), low).stream()
                .filter(range -> range.end() > range.start())
                .map(range -> new FoldingDescriptor(root.getNode(), new TextRange(range.start(), range.end())))
                .toArray(FoldingDescriptor[]::new);
    }
    @Override public String getPlaceholderText(@NotNull ASTNode node) { return " … "; }
    @Override public boolean isCollapsedByDefault(@NotNull ASTNode node) { return false; }
}
