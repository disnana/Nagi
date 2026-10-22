package com.disnana.nagi;

import com.intellij.lang.documentation.AbstractDocumentationProvider;
import com.intellij.openapi.editor.Editor;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** Quick Documentation and mouse hover share the current compiler-owned location graph. */
public final class NagiDocumentationProvider extends AbstractDocumentationProvider {
    @Override public @Nullable PsiElement getCustomDocumentationElement(@NotNull Editor editor, @NotNull PsiFile file,
                                                                       @Nullable PsiElement contextElement, int targetOffset) {
        // Mouse hover can be far from the caret. Never substitute the editor's
        // current caret or a similarly spelled declaration for this offset.
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())
                || targetOffset < 0 || targetOffset >= file.getTextLength()) return null;
        return sourceElement(file.findElementAt(targetOffset));
    }

    @Override public @Nullable String generateDoc(PsiElement element, @Nullable PsiElement originalElement) {
        PsiElement source = sourceElement(originalElement);
        if (source == null) source = sourceElement(element);
        if (source == null) return null;
        PsiFile file = source.getContainingFile();
        NagiAssistService service = file.getProject().getService(NagiAssistService.class);
        if (service == null) return null;
        var response = service.fresh(file);
        if (response == null) {
            // Nonblocking warmup only; a later hover/Quick Documentation retry
            // can display the result after the normal worker publishes it.
            service.requestNavigation(file);
            return null;
        }
        return service.documentationHtml(file, source);
    }

    @Override public @Nullable String getQuickNavigateInfo(PsiElement element, PsiElement originalElement) {
        return generateDoc(element, originalElement);
    }

    static @Nullable PsiElement sourceElement(@Nullable PsiElement element) {
        if (element == null || !element.isValid()) return null;
        PsiFile file = element.getContainingFile();
        if (file == null || !NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        if (element instanceof NagiParserDefinition.ReferenceElement) return element;
        return element.getParent() instanceof NagiParserDefinition.ReferenceElement ? element.getParent() : null;
    }
}
