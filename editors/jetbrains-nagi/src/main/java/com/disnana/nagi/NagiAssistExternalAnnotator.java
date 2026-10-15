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

/** Renders structured diagnostics and resolved symbol roles from a fresh compiler response. */
public final class NagiAssistExternalAnnotator extends ExternalAnnotator<NagiAssistExternalAnnotator.Work, NagiAssistExternalAnnotator.Work>
        implements DumbAware {
    record Work(PsiFile file, NagiAssistService service, NagiAssistService.CachedResponse response,
                NagiAssistService.State state, String message, long documentStamp) {}

    private static Work work(PsiFile file, NagiAssistService service, NagiAssistService.CachedResponse response) {
        Document document = PsiDocumentManager.getInstance(file.getProject()).getDocument(file);
        return document == null ? null : new Work(file, service, response, service.state(), service.statusMessage(), document.getModificationStamp());
    }

    @Override public @Nullable Work collectInformation(@NotNull PsiFile file) {
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        NagiAssistService service = file.getProject().getService(NagiAssistService.class);
        if (service == null) return null;
        NagiAssistService.CachedResponse response = service.fresh(file);
        if (response == null) {
            service.requestBaseline(file);
        }
        return work(file, service, response);
    }

    @Override public @Nullable Work collectInformation(@NotNull PsiFile file, @NotNull Editor editor, boolean hasErrors) {
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        NagiAssistService service = file.getProject().getService(NagiAssistService.class);
        if (service == null) return null;
        NagiAssistService.CachedResponse response = service.fresh(file);
        if (response == null) {
            service.requestBaseline(file, editor);
        }
        return work(file, service, response);
    }

    @Override public @Nullable Work doAnnotate(Work collectedInfo) { return collectedInfo; }

    @Override public void apply(@NotNull PsiFile file, Work result, @NotNull AnnotationHolder holder) {
        if (result == null || result.file() != file || !file.isValid()) return;
        Document document = PsiDocumentManager.getInstance(file.getProject()).getDocument(file);
        if (document == null) return;
        if (result.response() == null) {
            if (document.getModificationStamp() != result.documentStamp()
                    || result.service().state() != result.state()
                    || !result.service().statusMessage().equals(result.message())) return;
            if (shouldRenderFileLevelState(result.state())) {
                holder.newAnnotation(HighlightSeverity.WEAK_WARNING,
                        "Nagi editor assistance: " + result.message()).fileLevel().create();
            }
            return;
        }
        if (!result.service().isCurrent(file, result.response())) return;
        for (NagiSemanticIndex.Color color : result.service().semanticColors(file, result.response())) {
            com.intellij.openapi.progress.ProgressManager.checkCanceled();
            var point = color.location();
            int start = lineColumnToOffset(document, point.line(), point.column());
            if (start < 0 || point.length() < 1 || point.length() > document.getTextLength() - start) continue;
            int end = start + point.length();
            if (end > document.getLineEndOffset(point.line() - 1)
                    || splitsSurrogatePair(document, start) || splitsSurrogatePair(document, end)) continue;
            var token = file.findElementAt(start);
            if (token == null || token.getTextRange().getStartOffset() != start
                    || token.getTextRange().getEndOffset() != end) continue;
            var kind = token.getNode().getElementType();
            if (kind != NagiTokens.get(NagiTokenScanner.Kind.IDENTIFIER)
                    && kind != NagiTokens.get(NagiTokenScanner.Kind.TYPE)) continue;
            holder.newSilentAnnotation(HighlightSeverity.INFORMATION).range(new TextRange(start, end))
                    .textAttributes(color.attributes()).create();
        }
        for (NagiAssistProtocol.Diagnostic diagnostic : result.response().response().diagnostics()) {
            if (!result.service().diagnosticTargetsFile(result.response(), file.getVirtualFile(), diagnostic.file())) continue;
            String message = diagnostic.message();
            if (message == null || message.isBlank() || message.length() > 4096) continue;
            // Unloaded/missing modules and virtual stdlib origins retain the
            // compiler's message, but never acquire a guessed local range.
            if (diagnostic.file() != null && !result.service().diagnosticOriginMatchesFile(
                    result.response(), file.getVirtualFile(), diagnostic.file())) {
                holder.newAnnotation(HighlightSeverity.ERROR, message).fileLevel().create();
                continue;
            }
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

    static boolean shouldRenderFileLevelState(NagiAssistService.State state) {
        return switch (state) {
            case UNTRUSTED, UNSAVED_MANIFEST, LIMIT, FAILED -> true;
            default -> false;
        };
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
