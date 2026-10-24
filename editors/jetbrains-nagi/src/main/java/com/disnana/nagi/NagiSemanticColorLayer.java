package com.disnana.nagi;

import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.ex.MarkupModelEx;
import com.intellij.openapi.editor.impl.DocumentMarkupModel;
import com.intellij.openapi.editor.markup.HighlighterLayer;
import com.intellij.openapi.editor.markup.HighlighterTargetArea;
import com.intellij.openapi.editor.markup.RangeHighlighter;
import com.intellij.psi.PsiFile;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;

/** Color-only markup, independent of the daemon's replaceable external-annotation pass. */
final class NagiSemanticColorLayer {
    private final IdentityHashMap<Document, List<RangeHighlighter>> documents = new IdentityHashMap<>();

    boolean replace(PsiFile file, Document document, List<NagiSemanticPresentation.Paint> paints,
                    java.util.function.BooleanSupplier current) {
        if (!current.getAsBoolean()) return false;
        var previous = documents.get(document);
        if (previous != null && previous.size() == paints.size()) {
            boolean equal = true;
            for (int index = 0; index < paints.size(); index++) {
                var highlighter = previous.get(index);
                var paint = paints.get(index);
                if (!highlighter.isValid() || highlighter.getStartOffset() != paint.range().getStartOffset()
                        || highlighter.getEndOffset() != paint.range().getEndOffset()
                        || highlighter.getTextAttributesKey() != paint.attributes()) { equal = false; break; }
            }
            if (equal) return true;
        }
        if (previous == null && documents.size() >= NagiAssistProtocol.MAX_FILES) return false;
        List<RangeHighlighter> replacement = new ArrayList<>(paints.size());
        try {
            var markup = (MarkupModelEx) DocumentMarkupModel.forDocument(document, file.getProject(), true);
            for (var paint : paints) {
                com.intellij.openapi.progress.ProgressManager.checkCanceled();
                // This callback precedes afterAdded notification. Own the
                // registered range even if a listener throws before return.
                markup.addRangeHighlighterAndChangeAttributes(paint.attributes(), paint.range().getStartOffset(),
                        paint.range().getEndOffset(), HighlighterLayer.ADDITIONAL_SYNTAX,
                        HighlighterTargetArea.EXACT_RANGE, false, replacement::add);
            }
            com.intellij.openapi.progress.ProgressManager.checkCanceled();
            if (!current.getAsBoolean()) {
                replacement.forEach(RangeHighlighter::dispose);
                return false;
            }
        } catch (RuntimeException | Error failure) {
            replacement.forEach(RangeHighlighter::dispose);
            throw failure;
        }
        // Install before removing the previous colors. No empty daemon result
        // can remove these highlighters, which carry no HighlightInfo/pass ID.
        if (replacement.isEmpty()) documents.remove(document);
        else documents.put(document, replacement);
        if (previous != null) previous.forEach(RangeHighlighter::dispose);
        return current.getAsBoolean();
    }

    void retain(Document document, List<NagiSemanticPresentation.Paint> paints) {
        var highlighters = documents.get(document);
        if (highlighters == null) return;
        var allowed = new HashSet<>(paints);
        highlighters.removeIf(highlighter -> {
            if (!highlighter.isValid() || !allowed.contains(new NagiSemanticPresentation.Paint(
                    new com.intellij.openapi.util.TextRange(highlighter.getStartOffset(), highlighter.getEndOffset()),
                    highlighter.getTextAttributesKey()))) {
                highlighter.dispose(); return true;
            }
            return false;
        });
        if (highlighters.isEmpty()) documents.remove(document);
    }

    void clear() {
        documents.values().forEach(highlighters -> highlighters.forEach(RangeHighlighter::dispose));
        documents.clear();
    }
}
