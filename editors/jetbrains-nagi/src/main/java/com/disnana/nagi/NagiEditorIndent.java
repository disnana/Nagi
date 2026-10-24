package com.disnana.nagi;

import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.highlighter.EditorHighlighter;
import com.intellij.psi.tree.IElementType;
import java.util.ArrayDeque;

/** Small, conservative Enter context. The editor's cached lexer supplies token boundaries. */
final class NagiEditorIndent {
    static final int MAX_SCAN_CHARS = 8192;
    private static final int MAX_SCAN_LINES = 96;
    private static final int MAX_SCAN_TOKENS = 4096;
    private record Closer(IElementType type, int offset) {}

    private NagiEditorIndent() {}

    /** A null result leaves the IDE's own indent in place. */
    static String afterLine(Document document, EditorHighlighter highlighter, int line, boolean low,
                            int indentSize, int continuationSize, int tabSize) {
        int start = document.getLineStartOffset(line);
        int end = document.getLineEndOffset(line);
        if (end - start > MAX_SCAN_CHARS || highlighter == null) return null;
        CharSequence text = document.getCharsSequence();
        String base = spacesForIndent(text, start, end, tabSize);
        if (base == null) return null;
        if (indentSize < 0 || continuationSize < 0 || indentSize > MAX_SCAN_CHARS || continuationSize > MAX_SCAN_CHARS) return null;
        int blockWidth = indentSize;
        int continuationWidth = continuationSize;

        NagiTokenScanner.Token first = null;
        NagiTokenScanner.Token last = null;
        boolean blank = true;
        for (var token = NagiTokenScanner.next(text, start, end, low); token != null;
             token = NagiTokenScanner.next(text, token.end(), end, low)) {
            if (token.kind() != NagiTokenScanner.Kind.SPACE) blank = false;
            if (token.kind() == NagiTokenScanner.Kind.SPACE || token.kind() == NagiTokenScanner.Kind.COMMENT) continue;
            if (first == null) first = token;
            last = token;
        }
        boolean block = !low && last != null && last.kind() == NagiTokenScanner.Kind.PUNCTUATION
                && endChar(text, last) == ':';
        boolean returns = !low && first != null && first.kind() == NagiTokenScanner.Kind.KEYWORD
                && tokenTextIs(text, first, "return");

        // A blank High line retains its indentation, regardless of how many
        // times Enter was pressed. Manual dedent, including column zero, wins.
        if (!low && blank) return base;

        if (first == null && base.isEmpty() && line > 0 && (low || !blank)) {
            for (int previous = line - 1; previous >= Math.max(0, line - 8); previous--) {
                int previousStart = document.getLineStartOffset(previous);
                if (start - previousStart > MAX_SCAN_CHARS) return null;
                int previousEnd = document.getLineEndOffset(previous);
                String previousIndent = spacesForIndent(text, previousStart, previousEnd, tabSize);
                if (previousIndent == null) return null;
                for (var token = NagiTokenScanner.next(text, previousStart, previousEnd, low); token != null;
                     token = NagiTokenScanner.next(text, token.end(), previousEnd, low)) {
                    if (token.kind() != NagiTokenScanner.Kind.SPACE && token.kind() != NagiTokenScanner.Kind.COMMENT) {
                        return afterLine(document, highlighter, previous, low, indentSize, continuationSize, tabSize);
                    }
                }
                if (!previousIndent.isEmpty()) return previousIndent;
            }
            return null;
        }

        if (end == 0) return block ? base + " ".repeat(blockWidth) : base;
        int firstLine = Math.max(0, line - MAX_SCAN_LINES);
        int lowerBound = Math.max(document.getLineStartOffset(firstLine), end - MAX_SCAN_CHARS);
        var pendingClosers = new ArrayDeque<Closer>();
        Integer closedAnchor = null;
        var iterator = highlighter.createIterator(end - 1);
        int examined = 0;
        boolean boundedOut = false;
        while (!iterator.atEnd()) {
            if (iterator.getStart() < lowerBound || ++examined > MAX_SCAN_TOKENS) {
                boundedOut = true;
                break;
            }
            IElementType type = iterator.getTokenType();
            if (isClosing(type, low)) pendingClosers.push(new Closer(type, iterator.getStart()));
            else if (isOpening(type, low)) {
                if (pendingClosers.isEmpty()) {
                    String anchor = spacesForIndent(text, document.getLineStartOffset(document.getLineNumber(iterator.getStart())),
                            iterator.getStart(), tabSize);
                    if (anchor == null) return null;
                    return anchor + " ".repeat(isLowBrace(type) ? blockWidth : continuationWidth);
                }
                Closer closer = pendingClosers.pop();
                if (!matches(type, closer.type())) return null;
                if (closer.offset() >= start && iterator.getStart() < start) closedAnchor = iterator.getStart();
            }
            iterator.retreat();
        }
        if (!pendingClosers.isEmpty() || boundedOut) return null;
        if (closedAnchor != null) {
            int anchorStart = document.getLineStartOffset(document.getLineNumber(closedAnchor));
            String anchor = spacesForIndent(text, anchorStart, closedAnchor, tabSize);
            if (anchor == null) return null;
            base = anchor;
            if (!low) returns |= startsWithReturn(text, anchorStart, document.getLineEndOffset(document.getLineNumber(closedAnchor)), low);
        }
        if (block) return base + " ".repeat(blockWidth);
        if (returns) return " ".repeat(Math.max(0, base.length() - blockWidth));
        return base;
    }

    private static String spacesForIndent(CharSequence text, int start, int end, int tabSize) {
        int width = 0;
        int tabWidth = Math.max(1, tabSize);
        for (int offset = start; offset < end; offset++) {
            char c = text.charAt(offset);
            if (c == ' ') width++;
            else if (c == '\t') width += tabWidth - width % tabWidth;
            else break;
            if (width > MAX_SCAN_CHARS) return null;
        }
        return " ".repeat(width);
    }

    private static char endChar(CharSequence text, NagiTokenScanner.Token token) {
        return text.charAt(token.end() - 1);
    }
    private static boolean tokenTextIs(CharSequence text, NagiTokenScanner.Token token, String word) {
        if (token.end() - token.start() != word.length()) return false;
        for (int index = 0; index < word.length(); index++) if (text.charAt(token.start() + index) != word.charAt(index)) return false;
        return true;
    }
    private static boolean startsWithReturn(CharSequence text, int start, int end, boolean low) {
        if (end - start > MAX_SCAN_CHARS) return false;
        for (var token = NagiTokenScanner.next(text, start, end, low); token != null;
             token = NagiTokenScanner.next(text, token.end(), end, low)) {
            if (token.kind() == NagiTokenScanner.Kind.SPACE) continue;
            return token.kind() == NagiTokenScanner.Kind.KEYWORD && tokenTextIs(text, token, "return");
        }
        return false;
    }
    private static boolean isLowBrace(IElementType type) { return type == NagiTokens.get(NagiTokenScanner.Kind.LBRACE); }
    private static boolean isOpening(IElementType type, boolean low) {
        return type == NagiTokens.get(NagiTokenScanner.Kind.LPAREN)
                || type == NagiTokens.get(NagiTokenScanner.Kind.LBRACKET)
                || low && isLowBrace(type);
    }
    private static boolean isClosing(IElementType type, boolean low) {
        return type == NagiTokens.get(NagiTokenScanner.Kind.RPAREN)
                || type == NagiTokens.get(NagiTokenScanner.Kind.RBRACKET)
                || low && type == NagiTokens.get(NagiTokenScanner.Kind.RBRACE);
    }
    private static boolean matches(IElementType open, IElementType close) {
        return open == NagiTokens.get(NagiTokenScanner.Kind.LPAREN) && close == NagiTokens.get(NagiTokenScanner.Kind.RPAREN)
                || open == NagiTokens.get(NagiTokenScanner.Kind.LBRACKET) && close == NagiTokens.get(NagiTokenScanner.Kind.RBRACKET)
                || open == NagiTokens.get(NagiTokenScanner.Kind.LBRACE) && close == NagiTokens.get(NagiTokenScanner.Kind.RBRACE);
    }
}
