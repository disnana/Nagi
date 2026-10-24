package com.disnana.nagi;

import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.RangeMarker;
import com.intellij.openapi.editor.colors.TextAttributesKey;
import com.intellij.openapi.editor.event.DocumentEvent;
import com.intellij.openapi.util.TextRange;
import com.intellij.psi.PsiFile;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/** Last painted role styles only. Never a source of semantic facts or diagnostics. */
final class NagiSemanticPresentation {
    static final int MAX_ROLES = 20_000;
    private static final int MAX_TOKEN_LENGTH = 256;
    record Paint(TextRange range, TextAttributesKey attributes) {}
    private record Token(String text, com.intellij.psi.tree.IElementType kind) {}
    private record Role(RangeMarker marker, Token token, TextAttributesKey attributes, boolean low) {}
    private final IdentityHashMap<Document, List<Role>> documents = new IdentityHashMap<>();
    private final NagiSemanticColorLayer colors = new NagiSemanticColorLayer();
    private final java.util.function.BiFunction<Document, TextRange, RangeMarker> markerFactory;
    private Object generation;
    private Document focus;
    private int roleCount;

    NagiSemanticPresentation() { this(Document::createRangeMarker); }
    NagiSemanticPresentation(java.util.function.BiFunction<Document, TextRange, RangeMarker> markerFactory) {
        this.markerFactory = markerFactory;
    }

    synchronized void published(Object generation, Document focus) {
        // A new response is not a paint. The existing color-only presentation
        // stays until its replacement (including an empty one) is actually painted.
        if (this.generation == null || this.focus != focus) clear();
        this.generation = generation;
        this.focus = focus;
    }

    synchronized boolean softEdit(Document document) { return document == focus; }

    synchronized void afterEdit(DocumentEvent event) {
        // Document markers have moved now, but PSI may still describe the old
        // text. Validate with the same lexical scanner as the editor lexer.
        Document document = event.getDocument();
        int first = Math.max(0, document.getLineNumber(event.getOffset()) - 1);
        int last = Math.min(document.getLineCount() - 1,
                document.getLineNumber(event.getOffset() + event.getNewLength()) + 1);
        // The editor lexer has no cross-line state. Only the changed physical
        // lines (and the joined/split boundaries) need lexical revalidation.
        retained(document, first, last, false);
    }

    synchronized void beforeEdit(DocumentEvent event) {
        if (event.getDocument() != focus) { clear(); return; }
        for (var iterator = documents.entrySet().iterator(); iterator.hasNext();) {
            var entry = iterator.next();
            if (entry.getKey() != focus) {
                entry.getValue().forEach(role -> role.marker().dispose());
                colors.retain(entry.getKey(), List.of());
                roleCount -= entry.getValue().size();
                iterator.remove();
                continue;
            }
            int start = event.getOffset(), end = start + event.getOldLength();
            entry.getValue().removeIf(role -> {
                var marker = role.marker();
                // Keep non-greedy markers at an edit boundary until afterEdit
                // proves the complete current token. Whitespace can preserve
                // it; identifier joining or comment/string conversion cannot.
                boolean touched = marker.isValid() && (event.getOldLength() == 0
                        ? start > marker.getStartOffset() && start < marker.getEndOffset()
                        : start < marker.getEndOffset() && end > marker.getStartOffset());
                if (!marker.isValid() || touched) {
                    marker.dispose(); roleCount--; return true;
                }
                return false;
            });
        }
    }

    synchronized List<Paint> current(PsiFile file, Document document, Object owner,
                                     List<NagiSemanticIndex.Color> colors) {
        return paint(file, document, owner, colors, true);
    }

    synchronized List<Paint> partial(PsiFile file, Document document, Object owner,
                                     List<NagiSemanticIndex.Color> colors) {
        return paint(file, document, owner, colors, false);
    }

    private List<Paint> paint(PsiFile file, Document document, Object owner,
                              List<NagiSemanticIndex.Color> colors, boolean complete) {
        if (owner == null || owner != generation) return List.of();
        var previous = documents.get(document);
        int previousCount = previous == null ? 0 : previous.size();
        List<Role> retained = new ArrayList<>();
        List<Paint> paints = new ArrayList<>();
        boolean low = file.getLanguage() == NagiLanguage.LOW;
        var tokens = new Tokens(document, low);
        boolean retain = previous != null || documents.size() < NagiAssistProtocol.MAX_FILES;
        int capacity = MAX_ROLES - (roleCount - previousCount);
        try {
            for (var color : colors) {
                com.intellij.openapi.progress.ProgressManager.checkCanceled();
                var point = color.location();
                int line = point.line() - 1;
                if (line < 0 || line >= document.getLineCount() || point.column() < 1 || point.length() < 1) continue;
                long start = (long) document.getLineStartOffset(line) + point.column() - 1;
                long end = start + point.length();
                if (start < 0 || end > document.getLineEndOffset(line)) continue;
                TextRange range = new TextRange((int) start, (int) end);
                var token = tokens.at(range);
                if (token == null) continue;
                paints.add(new Paint(range, color.attributes()));
            }
            if (!complete && previous != null) {
                var merged = new java.util.LinkedHashMap<TextRange, Paint>();
                for (var paint : paints) merged.put(paint.range(), paint);
                for (var role : previous) {
                    com.intellij.openapi.progress.ProgressManager.checkCanceled();
                    var marker = role.marker();
                    if (!marker.isValid()) continue;
                    var range = new TextRange(marker.getStartOffset(), marker.getEndOffset());
                    if (role.token().equals(tokens.at(range))) {
                        // Every accepted span is a full lexical token, so
                        // overlapping spans have equal ranges. New facts win.
                        merged.putIfAbsent(range, new Paint(range, role.attributes()));
                    }
                }
                paints = new ArrayList<>(merged.values());
            }
            for (var paint : paints) {
                com.intellij.openapi.progress.ProgressManager.checkCanceled();
                var range = paint.range();
                var token = tokens.at(range);
                if (retain && retained.size() < capacity && token.text().length() <= MAX_TOKEN_LENGTH) {
                    retained.add(new Role(markerFactory.apply(document, range), token, paint.attributes(), low));
                }
            }
            com.intellij.openapi.progress.ProgressManager.checkCanceled();
            if (owner != generation || !this.colors.replace(file, document, paints, () -> owner == generation)
                    || owner != generation) {
                retained.forEach(role -> role.marker().dispose());
                return List.of();
            }
        } catch (RuntimeException | Error failure) {
            retained.forEach(role -> role.marker().dispose());
            throw failure;
        }
        // All fallible/cancellable preparation precedes this commit. At most
        // one old and one bounded replacement marker set coexist temporarily.
        if (!retained.isEmpty()) documents.put(document, retained);
        else documents.remove(document);
        roleCount += retained.size() - previousCount;
        if (previous != null) previous.forEach(role -> role.marker().dispose());
        return List.copyOf(paints);
    }

    synchronized List<Paint> retained(PsiFile file, Document document) {
        return retained(document);
    }

    private List<Paint> retained(Document document) {
        return retained(document, 0, document.getLineCount() - 1, true);
    }

    private List<Paint> retained(Document document, int first, int last, boolean cancellable) {
        List<Role> roles = documents.get(document);
        if (roles == null) { colors.retain(document, List.of()); return List.of(); }
        List<Paint> paints = new ArrayList<>();
        Map<Boolean, Tokens> tokens = new HashMap<>();
        for (var iterator = roles.iterator(); iterator.hasNext();) {
            if (cancellable) com.intellij.openapi.progress.ProgressManager.checkCanceled();
            var role = iterator.next();
            RangeMarker marker = role.marker();
            TextRange range = marker.isValid() ? new TextRange(marker.getStartOffset(), marker.getEndOffset()) : null;
            int line = range == null || range.getStartOffset() > document.getTextLength() ? -1
                    : document.getLineNumber(range.getStartOffset());
            if (range == null || line < 0 || (line >= first && line <= last && !role.token().equals(
                    tokens.computeIfAbsent(role.low(), low -> new Tokens(document, low, cancellable)).at(range)))) {
                marker.dispose(); roleCount--; iterator.remove();
            } else {
                paints.add(new Paint(range, role.attributes()));
            }
        }
        colors.retain(document, paints);
        return List.copyOf(paints);
    }

    /** One temporary lexical index per inspected physical line; never semantic facts. */
    private static final class Tokens {
        private final Document document;
        private final boolean low;
        private final boolean cancellable;
        private final Map<Integer, Map<TextRange, Token>> lines = new HashMap<>();
        Tokens(Document document, boolean low) { this(document, low, true); }
        Tokens(Document document, boolean low, boolean cancellable) {
            this.document = document; this.low = low; this.cancellable = cancellable;
        }
        Token at(TextRange range) {
            if (range.isEmpty() || range.getEndOffset() > document.getTextLength()) return null;
            int line = document.getLineNumber(range.getStartOffset());
            return lines.computeIfAbsent(line, this::line).get(range);
        }
        private Map<TextRange, Token> line(int line) {
            Map<TextRange, Token> tokens = new HashMap<>();
            CharSequence text = document.getCharsSequence();
            int end = document.getLineEndOffset(line);
            for (int offset = document.getLineStartOffset(line); offset < end;) {
                if (cancellable) com.intellij.openapi.progress.ProgressManager.checkCanceled();
                var token = NagiTokenScanner.next(text, offset, end, low);
                if (token.kind() == NagiTokenScanner.Kind.IDENTIFIER || token.kind() == NagiTokenScanner.Kind.TYPE) {
                    tokens.put(new TextRange(token.start(), token.end()), new Token(
                            text.subSequence(token.start(), token.end()).toString(), NagiTokens.get(token.kind())));
                }
                offset = token.end();
            }
            return tokens;
        }
    }

    synchronized int retainedRoleCount() { return roleCount; }

    synchronized void clear() {
        colors.clear();
        documents.values().forEach(roles -> roles.forEach(role -> role.marker().dispose()));
        documents.clear();
        roleCount = 0;
        generation = null;
        focus = null;
    }
}
