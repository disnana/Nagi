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
    NagiAssistReference(@NotNull PsiElement element) {
        super(element, new TextRange(0, element.getTextLength()), true);
    }

    @Override public @Nullable PsiElement resolve() {
        com.intellij.openapi.progress.ProgressManager.checkCanceled();
        PsiElement result = resolveCurrent();
        com.intellij.openapi.progress.ProgressManager.checkCanceled();
        return result;
    }

    private @Nullable PsiElement resolveCurrent() {
        if (!myElement.isValid()) return null;
        PsiFile source = myElement.getContainingFile();
        if (source == null || source.getProject().isDisposed()) return null;
        NagiAssistService service = source.getProject().getService(NagiAssistService.class);
        return service == null ? null : service.resolveReference(source, myElement);
    }

    @Override public Object @NotNull [] getVariants() { return new Object[0]; }

    @Override public @NotNull PsiElement handleElementRename(@NotNull String newElementName) {
        throw new IncorrectOperationException("Nagi compiler references currently support navigation only.");
    }

    @Override public @NotNull PsiElement bindToElement(@NotNull PsiElement element) {
        throw new IncorrectOperationException("Nagi compiler references currently support navigation only.");
    }
}
