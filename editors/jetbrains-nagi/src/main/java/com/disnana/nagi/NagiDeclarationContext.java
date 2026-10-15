package com.disnana.nagi;

import java.util.function.LongSupplier;

/** Conservative, finite lexical check for top-level declaration keyword completion. */
final class NagiDeclarationContext {
    static final int MAX_PREFIX_BYTES = 8192;
    static final int MAX_TOKENS = 1024;
    private static final int MAX_NESTING = 256;
    private static final long MAX_SCAN_NANOS = 10_000_000L;

    private NagiDeclarationContext() {}

    static boolean isCertainTopLevel(CharSequence text, int offset, boolean low) {
        return isCertainTopLevel(text, offset, low, System::nanoTime);
    }

    // The clock overload makes expiration a deterministic test, independent of CI load.
    static boolean isCertainTopLevel(CharSequence text, int offset, boolean low, LongSupplier clock) {
        // UTF-8 byte length is checked while visiting the bounded token prefix.
        // UTF-16 offset > the byte cap can never fit, even for ASCII.
        if (offset < 0 || offset > text.length() || offset > MAX_PREFIX_BYTES) return false;
        long deadline = clock.getAsLong() + MAX_SCAN_NANOS;
        char[] open = new char[MAX_NESTING];
        int depth = 0;
        int count = 0;
        int bytes = 0;
        for (NagiTokenScanner.Token token = NagiTokenScanner.next(text, 0, offset, low);
             token != null; token = NagiTokenScanner.next(text, token.end(), offset, low)) {
            if (++count > MAX_TOKENS || (count % 64 == 0 && clock.getAsLong() - deadline >= 0)) return false;
            int spanBytes = utf8Bytes(text, token.start(), token.end(), MAX_PREFIX_BYTES - bytes);
            if (spanBytes < 0) return false;
            bytes += spanBytes;
            if (rawLikePrefix(text, token, offset)) return false;
            switch (token.kind()) {
                case STRING -> {
                    if (!closedString(text, token.start(), token.end())) return false;
                }
                case LPAREN, LBRACKET, LBRACE -> {
                    if (depth == MAX_NESTING) return false;
                    open[depth++] = text.charAt(token.start());
                }
                case RPAREN, RBRACKET, RBRACE -> {
                    if (depth == 0 || !matches(open[--depth], text.charAt(token.start()))) return false;
                }
                case BAD -> { return false; }
                default -> { }
            }
        }
        return depth == 0;
    }

    private static boolean matches(char opening, char closing) {
        return opening == '(' && closing == ')' || opening == '[' && closing == ']'
                || opening == '{' && closing == '}';
    }

    private static int utf8Bytes(CharSequence text, int start, int end, int remaining) {
        int bytes = 0;
        for (int index = start; index < end; index++) {
            char value = text.charAt(index);
            if (value < 0x80) bytes++;
            else if (value < 0x800) bytes += 2;
            else if (Character.isHighSurrogate(value)) {
                if (index + 1 >= end || !Character.isLowSurrogate(text.charAt(index + 1))) return -1;
                bytes += 4;
                index++;
            } else if (Character.isLowSurrogate(value)) return -1;
            else bytes += 3;
            if (bytes > remaining) return -1;
        }
        return bytes;
    }

    private static boolean rawLikePrefix(CharSequence text, NagiTokenScanner.Token token, int end) {
        if (token.kind() != NagiTokenScanner.Kind.IDENTIFIER || token.end() >= end) return false;
        String word = text.subSequence(token.start(), token.end()).toString();
        if (!word.equals("r") && !word.equals("R") && !word.equals("br") && !word.equals("rb")
                && !word.equals("b") && !word.equals("B")) return false;
        char next = text.charAt(token.end());
        return next == '\'' || next == '"' || next == '#';
    }

    private static boolean closedString(CharSequence text, int start, int end) {
        if (end - start < 2) return false;
        char quote = text.charAt(start);
        for (int index = start + 1; index < end; index++) {
            char value = text.charAt(index);
            if (value == '\\') {
                if (++index >= end) return false;
            } else if (value == quote) {
                return index == end - 1;
            } else if (value == '\n' || value == '\r') {
                return false;
            }
        }
        return false;
    }
}
