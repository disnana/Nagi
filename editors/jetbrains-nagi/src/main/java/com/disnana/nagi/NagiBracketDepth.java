package com.disnana.nagi;

import com.intellij.openapi.util.TextRange;
import com.intellij.openapi.progress.ProgressManager;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** Linear, lexical bracket pairing. It never treats text in strings or comments as code. */
final class NagiBracketDepth {
    static final int PALETTE_SIZE = 6;
    static final int MAX_PAIRS = 20_000;
    record Span(TextRange range, int paletteIndex) {}
    private record Opening(NagiTokenScanner.Kind kind, int offset, int paletteIndex) {}

    private NagiBracketDepth() {}

    static List<Span> spans(CharSequence text, boolean low) {
        var openings = new ArrayDeque<Opening>();
        var spans = new ArrayList<Span>();
        int pairs = 0;
        int tokens = 0;
        for (var token = NagiTokenScanner.next(text, 0, text.length(), low); token != null;
             token = NagiTokenScanner.next(text, token.end(), text.length(), low)) {
            if ((tokens++ & 255) == 0) ProgressManager.checkCanceled();
            if (opening(token.kind(), low)) {
                if (openings.size() >= MAX_PAIRS) return List.of();
                openings.push(new Opening(token.kind(), token.start(), openings.size() % PALETTE_SIZE));
                continue;
            }
            if (!closing(token.kind(), low)) continue;
            if (openings.isEmpty() || !matches(openings.peek().kind(), token.kind())) {
                // Do not invent a pair across malformed syntax. The normal syntax
                // highlighter remains visible for every unmatched bracket.
                openings.clear();
                continue;
            }
            Opening opening = openings.pop();
            if (++pairs > MAX_PAIRS) return List.of();
            spans.add(new Span(new TextRange(opening.offset(), opening.offset() + 1), opening.paletteIndex()));
            spans.add(new Span(new TextRange(token.start(), token.end()), opening.paletteIndex()));
        }
        spans.sort(Comparator.comparingInt(span -> span.range().getStartOffset()));
        return List.copyOf(spans);
    }

    private static boolean opening(NagiTokenScanner.Kind kind, boolean low) {
        return kind == NagiTokenScanner.Kind.LPAREN || kind == NagiTokenScanner.Kind.LBRACKET
                || low && kind == NagiTokenScanner.Kind.LBRACE;
    }
    private static boolean closing(NagiTokenScanner.Kind kind, boolean low) {
        return kind == NagiTokenScanner.Kind.RPAREN || kind == NagiTokenScanner.Kind.RBRACKET
                || low && kind == NagiTokenScanner.Kind.RBRACE;
    }
    private static boolean matches(NagiTokenScanner.Kind open, NagiTokenScanner.Kind close) {
        return open == NagiTokenScanner.Kind.LPAREN && close == NagiTokenScanner.Kind.RPAREN
                || open == NagiTokenScanner.Kind.LBRACKET && close == NagiTokenScanner.Kind.RBRACKET
                || open == NagiTokenScanner.Kind.LBRACE && close == NagiTokenScanner.Kind.RBRACE;
    }
}
