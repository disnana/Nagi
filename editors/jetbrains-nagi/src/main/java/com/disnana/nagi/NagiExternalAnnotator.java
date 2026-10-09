package com.disnana.nagi;

import com.intellij.lang.annotation.AnnotationHolder;
import com.intellij.lang.annotation.ExternalAnnotator;
import com.intellij.lang.annotation.HighlightSeverity;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.util.TextRange;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** Asynchronous diagnostics; application rechecks the exact source snapshot. */
public final class NagiExternalAnnotator extends ExternalAnnotator<NagiExternalAnnotator.Input, NagiExternalAnnotator.Input> {
    public record Input(PsiFile file, NagiAssistService service, NagiAssistService.CachedResponse snapshot) {}

    @Override public @Nullable Input collectInformation(@NotNull PsiFile file, @NotNull Editor editor,
                                                        boolean hasErrors) {
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        var service = file.getProject().getService(NagiAssistService.class);
        var snapshot = service.fresh(file);
        if (snapshot == null) {
            service.requestBaseline(file, editor);
            return null;
        }
        return new Input(file, service, snapshot);
    }

    @Override public @Nullable Input doAnnotate(Input input) { return input; }

    @Override public void apply(@NotNull PsiFile file, Input input, @NotNull AnnotationHolder holder) {
        if (input == null || input.file() != file || !input.service().isCurrent(file, input.snapshot())) return;
        Document document = PsiDocumentManager.getInstance(file.getProject()).getDocument(file);
        if (document == null) return;
        for (var diagnostic : input.snapshot().response().diagnostics()) {
            if (!input.service().diagnosticTargetsFile(input.snapshot(), file.getVirtualFile(), diagnostic.file())) continue;
            TextRange range = diagnosticRange(document, diagnostic);
            if (range != null) holder.newAnnotation(HighlightSeverity.ERROR, diagnostic.message()).range(range).create();
        }
    }

    static @Nullable TextRange diagnosticRange(Document document, NagiAssistProtocol.Diagnostic diagnostic) {
        if (diagnostic.line() == null) return null; // A global message has no invented file position.
        int line = diagnostic.line() - 1;
        if (line < 0 || line >= document.getLineCount()) return null;
        int start = document.getLineStartOffset(line);
        int end = document.getLineEndOffset(line);
        var point = diagnostic.range();
        if (point != null) {
            if (point.line() != diagnostic.line()) return null;
            long offset = (long) start + point.column() - 1;
            long limit = offset + point.length();
            if (offset < start || limit > end || splitsSurrogate(document, (int) offset)
                    || splitsSurrogate(document, (int) limit)) return null;
            return new TextRange((int) offset, (int) limit);
        }
        return new TextRange(start, end); // The compiler reports a line, not a guessed token span.
    }

    private static boolean splitsSurrogate(Document document, int offset) {
        if (offset < 1 || offset >= document.getTextLength()) return false;
        var text = document.getCharsSequence();
        return Character.isHighSurrogate(text.charAt(offset - 1)) && Character.isLowSurrogate(text.charAt(offset));
    }
}
