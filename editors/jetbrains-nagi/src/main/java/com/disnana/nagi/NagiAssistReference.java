package com.disnana.nagi;

import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.PsiReferenceBase;
import com.intellij.util.IncorrectOperationException;
import com.intellij.openapi.util.TextRange;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** A soft, navigation-only compiler reference; it never performs IDE-side name resolution. */
final class NagiAssistReference extends PsiReferenceBase<PsiElement> {
    private final NagiAssistService service;
    private final NagiAssistService.CachedResponse response;
    private final NagiAssistProtocol.Location target;

    NagiAssistReference(@NotNull PsiElement element, NagiAssistService service,
                        NagiAssistService.CachedResponse response, NagiAssistProtocol.Location target) {
        super(element, new TextRange(0, element.getTextLength()), true);
        this.service = service;
        this.response = response;
        this.target = target;
    }

    @Override public @Nullable PsiElement resolve() {
        PsiFile source = myElement.getContainingFile();
        if (source == null || !myElement.isValid()) return null;
        return service.resolveTarget(source, response, target);
    }

    @Override public Object @NotNull [] getVariants() { return new Object[0]; }

    @Override public @NotNull PsiElement handleElementRename(@NotNull String newElementName) {
        throw new IncorrectOperationException("Nagi compiler references currently support navigation only.");
    }

    @Override public @NotNull PsiElement bindToElement(@NotNull PsiElement element) {
        throw new IncorrectOperationException("Nagi compiler references currently support navigation only.");
    }
}
