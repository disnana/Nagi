package com.disnana.nagi;

import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer;
import com.intellij.lang.annotation.AnnotationHolder;
import com.intellij.lang.annotation.ExternalAnnotator;
import com.intellij.lang.annotation.HighlightSeverity;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.project.DumbAware;
import com.intellij.openapi.util.TextRange;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import java.util.List;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** Renders only structured diagnostics from a fresh compiler response. */
public final class NagiAssistExternalAnnotator extends ExternalAnnotator<NagiAssistExternalAnnotator.Work, NagiAssistExternalAnnotator.Work>
        implements DumbAware {
    record Work(PsiFile file, NagiAssistService service, NagiAssistService.CachedResponse response) {}

    @Override public @Nullable Work collectInformation(@NotNull PsiFile file) {
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        NagiAssistService service = file.getProject().getService(NagiAssistService.class);
        if (service == null) return null;
        NagiAssistService.CachedResponse response = service.fresh(file);
        if (response == null) {
            service.requestBaseline(file);
            return null;
        }
        return new Work(file, service, response);
    }

    @Override public @Nullable Work collectInformation(@NotNull PsiFile file, @NotNull Editor editor, boolean hasErrors) {
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        NagiAssistService service = file.getProject().getService(NagiAssistService.class);
        if (service == null) return null;
        NagiAssistService.CachedResponse response = service.fresh(file);
        if (response == null) {
            service.requestBaseline(file, editor);
            return null;
        }
        return new Work(file, service, response);
    }

    @Override public @Nullable Work doAnnotate(Work collectedInfo) { return collectedInfo; }

    @Override public void apply(@NotNull PsiFile file, Work result, @NotNull AnnotationHolder holder) {
        if (result == null || result.file() != file || !file.isValid()
                || !result.service().isCurrent(file, result.response())) return;
        Document document = PsiDocumentManager.getInstance(file.getProject()).getDocument(file);
        if (document == null) return;
        for (NagiAssistProtocol.Diagnostic diagnostic : result.response().response().diagnostics()) {
            if (!result.service().diagnosticTargetsFile(result.response(), file, diagnostic.file())) continue;
            String message = diagnostic.message();
            if (message == null || message.isBlank() || message.length() > 4096) continue;
            NagiAssistProtocol.Location point = diagnostic.range();
            if (point != null) {
                int start = lineColumnToOffset(document, point.line(), point.column());
                if (start < 0 || point.length() > document.getTextLength() - start) continue;
                int end = start + point.length();
                if (splitsSurrogatePair(document, start) || splitsSurrogatePair(document, end)) continue;
                int line = point.line() - 1;
                if (line < 0 || line >= document.getLineCount() || end > document.getLineEndOffset(line)) continue;
                var annotation = holder.newAnnotation(HighlightSeverity.ERROR, message);
                if (point.length() == 0 && start == document.getLineEndOffset(line)) {
                    annotation.afterEndOfLine().create();
                } else {
                    annotation.range(new TextRange(start, end)).create();
                }
                continue;
            }
            Integer line = diagnostic.line();
            if (line == null) {
                holder.newAnnotation(HighlightSeverity.ERROR, message).fileLevel().create();
                continue;
            }
            int lineIndex = line - 1;
            if (lineIndex < 0 || lineIndex >= document.getLineCount()) continue;
            int start = document.getLineStartOffset(lineIndex);
            int end = document.getLineEndOffset(lineIndex);
            var annotation = holder.newAnnotation(HighlightSeverity.ERROR, message);
            if (start == end) annotation.afterEndOfLine().create();
            else annotation.range(new TextRange(start, end)).create();
        }
    }

    private static int lineColumnToOffset(Document document, int line, int column) {
        int index = line - 1;
        if (index < 0 || index >= document.getLineCount() || column < 1) return -1;
        int start = document.getLineStartOffset(index);
        int end = document.getLineEndOffset(index);
        long offset = (long) start + column - 1;
        return offset >= start && offset <= end ? (int) offset : -1;
    }

    private static boolean splitsSurrogatePair(Document document, int offset) {
        CharSequence text = document.getCharsSequence();
        return offset > 0 && offset < text.length()
                && Character.isHighSurrogate(text.charAt(offset - 1))
                && Character.isLowSurrogate(text.charAt(offset));
    }
}
