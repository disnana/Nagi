package com.disnana.nagi;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/** Recognize runnable declarations without duplicating the compiler's type checker. */
final class NagiEntryPoints {
    private NagiEntryPoints() {}

    static Set<Integer> mainOffsets(CharSequence source, boolean low) {
        var tokens = new ArrayList<NagiTokenScanner.Token>();
        for (var token = NagiTokenScanner.next(source, 0, source.length(), low); token != null;
             token = NagiTokenScanner.next(source, token.end(), source.length(), low)) {
            if (token.kind() != NagiTokenScanner.Kind.SPACE && token.kind() != NagiTokenScanner.Kind.COMMENT) tokens.add(token);
        }
        var result = new LinkedHashSet<Integer>();
        var delimiters = new ArrayDeque<NagiTokenScanner.Kind>();
        for (int index = 0; index < tokens.size(); index++) {
            var token = tokens.get(index);
            if (delimiters.isEmpty() && declarationStart(source, tokens, index, low)) {
                int name = mainName(source, tokens, index, low);
                if (name >= 0) result.add(tokens.get(name).start());
            }
            switch (token.kind()) {
                case LPAREN, LBRACKET, LBRACE -> delimiters.push(token.kind());
                case RPAREN, RBRACKET, RBRACE -> {
                    if (!delimiters.isEmpty() && matches(delimiters.peek(), token.kind())) delimiters.pop();
                    else delimiters.push(token.kind()); // An incomplete outer expression cannot expose a nested main.
                }
                default -> {}
            }
        }
        return Set.copyOf(result);
    }

    private static boolean declarationStart(CharSequence source, List<NagiTokenScanner.Token> tokens, int index, boolean low) {
        if (index > 0 && word(source, tokens.get(index - 1)).equals("async")) return false;
        int offset = tokens.get(index).start();
        int start = offset;
        while (start > 0 && source.charAt(start - 1) != '\n' && source.charAt(start - 1) != '\r') start--;
        if (!low) return start == offset;
        for (int position = start; position < offset; position++) {
            if (!Character.isWhitespace(source.charAt(position))) {
                if (index == 0) return false;
                var previous = tokens.get(index - 1);
                return previous.kind() == NagiTokenScanner.Kind.RBRACE || word(source, previous).equals(";");
            }
        }
        return true;
    }

    private static int mainName(CharSequence source, List<NagiTokenScanner.Token> tokens, int index, boolean low) {
        int declaration = index;
        if (word(source, tokens.get(declaration)).equals("async")) declaration++;
        if (declaration + 3 >= tokens.size()) return -1;
        var keyword = tokens.get(declaration);
        var name = tokens.get(declaration + 1);
        if (keyword.kind() != NagiTokenScanner.Kind.KEYWORD || !word(source, keyword).equals(low ? "fn" : "def")
                || name.kind() != NagiTokenScanner.Kind.IDENTIFIER || !word(source, name).equals("main")
                || !sameLine(source, tokens.get(index).start(), name.end())
                || tokens.get(declaration + 2).kind() != NagiTokenScanner.Kind.LPAREN
                || tokens.get(declaration + 3).kind() != NagiTokenScanner.Kind.RPAREN) return -1;
        int next = declaration + 4;
        if (next >= tokens.size()) return -1;
        if (word(source, tokens.get(next)).equals("->")) {
            next++;
            int typeStart = next;
            var delimiters = new ArrayDeque<NagiTokenScanner.Kind>();
            for (; next < tokens.size(); next++) {
                var token = tokens.get(next);
                if (!low && delimiters.isEmpty() && !sameLine(source, tokens.get(next - 1).end(), token.start())) return -1;
                if (delimiters.isEmpty() && bodyStart(source, token, low)) break;
                if (word(source, token).equals("fn")
                        && (next + 1 >= tokens.size() || tokens.get(next + 1).kind() != NagiTokenScanner.Kind.LPAREN
                        && tokens.get(next + 1).kind() != NagiTokenScanner.Kind.LBRACKET)) return -1;
                if (token.kind() == NagiTokenScanner.Kind.IDENTIFIER || token.kind() == NagiTokenScanner.Kind.TYPE
                        || token.kind() == NagiTokenScanner.Kind.KEYWORD && word(source, token).equals("fn")) continue;
                switch (token.kind()) {
                    case LPAREN, LBRACKET -> delimiters.push(token.kind());
                    case RPAREN, RBRACKET -> {
                        if (delimiters.isEmpty() || !matches(delimiters.pop(), token.kind())) return -1;
                    }
                    case OPERATOR, PUNCTUATION -> {
                        if (!Set.of("?", ",", ".", "->", ":").contains(word(source, token))) return -1;
                    }
                    default -> { return -1; }
                }
            }
            if (next == typeStart || !delimiters.isEmpty()) return -1;
        }
        return next < tokens.size() && bodyStart(source, tokens.get(next), low)
                && (low || sameLine(source, tokens.get(next - 1).end(), tokens.get(next).start())) ? declaration + 1 : -1;
    }

    private static boolean bodyStart(CharSequence source, NagiTokenScanner.Token token, boolean low) {
        return low ? token.kind() == NagiTokenScanner.Kind.LBRACE : word(source, token).equals(":");
    }
    private static boolean sameLine(CharSequence source, int start, int end) {
        for (int index = start; index < end; index++) if (source.charAt(index) == '\n' || source.charAt(index) == '\r') return false;
        return true;
    }
    private static boolean matches(NagiTokenScanner.Kind open, NagiTokenScanner.Kind close) {
        return open == NagiTokenScanner.Kind.LPAREN && close == NagiTokenScanner.Kind.RPAREN
                || open == NagiTokenScanner.Kind.LBRACKET && close == NagiTokenScanner.Kind.RBRACKET
                || open == NagiTokenScanner.Kind.LBRACE && close == NagiTokenScanner.Kind.RBRACE;
    }
    private static String word(CharSequence source, NagiTokenScanner.Token token) {
        return source.subSequence(token.start(), token.end()).toString();
    }
}
