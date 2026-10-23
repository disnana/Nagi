package com.disnana.nagi;

import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.RangeMarker;
import com.intellij.openapi.editor.colors.TextAttributesKey;
import com.intellij.openapi.editor.event.DocumentEvent;
import com.intellij.openapi.util.TextRange;
import com.intellij.psi.PsiFile;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;

/** Last painted role styles only. Never a source of semantic facts or diagnostics. */
final class NagiSemanticPresentation {
    static final int MAX_ROLES = 20_000;
    private static final int MAX_TOKEN_LENGTH = 256;
    record Paint(TextRange range, TextAttributesKey attributes) {}
    private record Token(String text, com.intellij.psi.tree.IElementType kind) {}
    private record Role(RangeMarker marker, Token token, TextAttributesKey attributes) {}
    private final IdentityHashMap<Document, List<Role>> documents = new IdentityHashMap<>();
    private Object generation;
    private Document focus;
    private int roleCount;

    synchronized void published(Object generation, Document focus) {
        clear();
        this.generation = generation;
        this.focus = focus;
    }

    synchronized boolean softEdit(Document document) { return document == focus; }

    synchronized void beforeEdit(DocumentEvent event) {
        if (event.getDocument() != focus) { clear(); return; }
        for (var iterator = documents.entrySet().iterator(); iterator.hasNext();) {
            var entry = iterator.next();
            if (entry.getKey() != focus) {
                entry.getValue().forEach(role -> role.marker().dispose());
                roleCount -= entry.getValue().size();
                iterator.remove();
                continue;
            }
            int start = event.getOffset(), end = start + event.getOldLength();
            entry.getValue().removeIf(role -> {
                var marker = role.marker();
                // Include boundaries: insertion next to an identifier can
                // change the token even with non-greedy range markers.
                if (!marker.isValid() || (start <= marker.getEndOffset() && end >= marker.getStartOffset())) {
                    marker.dispose(); roleCount--; return true;
                }
                return false;
            });
        }
    }

    synchronized List<Paint> current(PsiFile file, Document document, Object owner,
                                     List<NagiSemanticIndex.Color> colors) {
        if (owner != generation) return List.of();
        var previous = documents.remove(document);
        if (previous != null) {
            previous.forEach(role -> role.marker().dispose());
            roleCount -= previous.size();
        }
        List<Role> retained = new ArrayList<>();
        List<Paint> paints = new ArrayList<>();
        boolean retain = documents.size() < NagiAssistProtocol.MAX_FILES;
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
                var token = token(file, range);
                if (token == null) continue;
                paints.add(new Paint(range, color.attributes()));
                if (retain && roleCount < MAX_ROLES && token.text().length() <= MAX_TOKEN_LENGTH) {
                    retained.add(new Role(document.createRangeMarker(range), token, color.attributes()));
                    roleCount++;
                }
            }
        } catch (RuntimeException | Error failure) {
            retained.forEach(role -> role.marker().dispose());
            roleCount -= retained.size();
            throw failure;
        }
        if (!retained.isEmpty()) documents.put(document, retained);
        return List.copyOf(paints);
    }

    synchronized List<Paint> retained(PsiFile file, Document document) {
        List<Role> roles = documents.get(document);
        if (roles == null) return List.of();
        List<Paint> paints = new ArrayList<>();
        for (var iterator = roles.iterator(); iterator.hasNext();) {
            com.intellij.openapi.progress.ProgressManager.checkCanceled();
            var role = iterator.next();
            RangeMarker marker = role.marker();
            TextRange range = marker.isValid() ? new TextRange(marker.getStartOffset(), marker.getEndOffset()) : null;
            if (range == null || !role.token().equals(token(file, range))) {
                marker.dispose(); roleCount--; iterator.remove();
            } else {
                paints.add(new Paint(range, role.attributes()));
            }
        }
        return List.copyOf(paints);
    }

    private static Token token(PsiFile file, TextRange range) {
        if (range.isEmpty() || range.getEndOffset() > file.getTextLength()) return null;
        var token = file.findElementAt(range.getStartOffset());
        if (token == null || !range.equals(token.getTextRange())) return null;
        var kind = token.getNode().getElementType();
        return kind == NagiTokens.get(NagiTokenScanner.Kind.IDENTIFIER) || kind == NagiTokens.get(NagiTokenScanner.Kind.TYPE)
                ? new Token(token.getText(), kind) : null;
    }

    synchronized int retainedRoleCount() { return roleCount; }

    synchronized void clear() {
        documents.values().forEach(roles -> roles.forEach(role -> role.marker().dispose()));
        documents.clear();
        roleCount = 0;
        generation = null;
        focus = null;
    }
}
