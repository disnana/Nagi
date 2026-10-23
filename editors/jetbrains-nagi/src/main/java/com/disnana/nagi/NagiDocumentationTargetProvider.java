package com.disnana.nagi;

import com.intellij.model.Pointer;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.progress.ProgressManager;
import com.intellij.openapi.progress.util.ProgressIndicatorUtils;
import com.intellij.platform.backend.documentation.DocumentationResult;
import com.intellij.platform.backend.documentation.DocumentationTarget;
import com.intellij.platform.backend.documentation.DocumentationTargetProvider;
import com.intellij.platform.backend.presentation.TargetPresentation;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.SmartPointerManager;
import com.intellij.psi.SmartPsiElementPointer;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import org.jetbrains.annotations.NotNull;

/** Standard asynchronous mouse/Quick Documentation; never waits while holding a read lock. */
public final class NagiDocumentationTargetProvider implements DocumentationTargetProvider {
    @Override public @NotNull List<? extends DocumentationTarget> documentationTargets(@NotNull PsiFile file, int offset) {
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile()) || offset < 0 || offset >= file.getTextLength()) return List.of();
        PsiElement source = NagiDocumentationProvider.sourceElement(file.findElementAt(offset));
        if (source == null || source.getTextLength() > 256) return List.of();
        Document document = PsiDocumentManager.getInstance(file.getProject()).getDocument(file);
        var service = file.getProject().getService(NagiAssistService.class);
        if (document == null || service == null) return List.of();
        return List.of(new Target(SmartPointerManager.createPointer(source), document, document.getModificationStamp(),
                service.documentationEpoch(), source.getTextOffset(), source.getTextLength(), source.getText(), service));
    }

    private record Target(SmartPsiElementPointer<PsiElement> pointer, Document document, long stamp, long epoch,
                          int start, int length, String label, NagiAssistService service) implements DocumentationTarget {
        private PsiElement source() {
            PsiElement source = pointer.getElement();
            return source != null && source.isValid() && source.getTextOffset() == start && source.getTextLength() == length
                    && service.documentationIdentityCurrent(source.getContainingFile(), document, stamp, epoch) ? source : null;
        }

        @Override public @NotNull Pointer<? extends DocumentationTarget> createPointer() {
            return () -> source() == null ? null : this;
        }

        @Override public @NotNull TargetPresentation computePresentation() { return TargetPresentation.builder(label).presentation(); }

        private String html() {
            PsiElement source = source();
            if (source == null) return null;
            return service.documentationHtml(source.getContainingFile(), source);
        }

        @Override public DocumentationResult computeDocumentation() {
            PsiElement source = source();
            if (source == null) return null;
            if (service.fresh(source.getContainingFile()) != null) {
                String html = html();
                return html == null ? null : DocumentationResult.documentation(html);
            }
            // The platform runs this supplier in background under its own
            // cancellable indicator, outside the target's initial read action.
            return DocumentationResult.asyncDocumentation(this::awaitDocumentation);
        }

        private DocumentationResult.Documentation awaitDocumentation() {
            ApplicationManager.getApplication().assertIsNonDispatchThread();
            if (ApplicationManager.getApplication().isReadAccessAllowed())
                throw new IllegalStateException("Nagi documentation cannot wait in a read action.");
            ProgressManager.checkCanceled();
            CompletableFuture<Boolean> signal = NagiPlatform.read(() -> {
                PsiElement source = source();
                return source == null ? CompletableFuture.completedFuture(false)
                        : service.documentationTicket(source.getContainingFile(), document, stamp, epoch);
            });
            try {
                ProgressManager.checkCanceled();
                boolean ready = ProgressIndicatorUtils.awaitWithCheckCanceled(signal);
                ProgressManager.checkCanceled();
                if (!ready) return null;
                String html = NagiPlatform.read(this::html);
                ProgressManager.checkCanceled();
                return html == null ? null : DocumentationResult.documentation(html);
            } finally {
                service.unregisterDocumentation(signal);
                signal.cancel(false); // This ticket only; the shared compiler work is untouched.
            }
        }
    }
}
