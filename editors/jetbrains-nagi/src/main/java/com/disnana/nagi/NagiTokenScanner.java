package com.disnana.nagi;

import java.util.Set;

/** Lexical tokens only. Semantic information remains the compiler's responsibility. */
public final class NagiTokenScanner {
    public enum Kind { SPACE, COMMENT, STRING, NUMBER, KEYWORD, TYPE, IDENTIFIER,
        LPAREN, RPAREN, LBRACKET, RBRACKET, LBRACE, RBRACE, OPERATOR, PUNCTUATION, BAD }
    public record Token(Kind kind, int start, int end) {}
    private static final Set<String> COMMON = Set.of("import", "from", "as", "extern", "enum", "async", "await",
            "return", "match", "case", "if", "else", "while", "for", "in", "with", "scope", "spawn",
            "try", "and", "or", "not", "true", "false", "True", "False", "Some", "None", "null", "Ok", "Err");
    private static final Set<String> HIGH = Set.of("def", "class");
    private static final Set<String> LOW = Set.of("fn", "record", "let");
    private static final Set<String> TYPES = Set.of("i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64",
            "f32", "f64", "bool", "str", "bytes", "unit", "List", "Result", "Error", "view", "shared",
            "UUID", "timestamp", "Option", "Map", "fn");
    static final int MAX_DECLARATION_GAP = 8192;
    static final int CODE = 0;
    static final int TYPE_NAME = 1;
    static final int ADJACENT_PREFIX = 2;
    private static final Set<String> OPERATORS = Set.of("->", "==", "!=", "<=", ">=", "+=", "-=", "*=");

    private NagiTokenScanner() {}

    public static Token next(CharSequence text, int start, int end, boolean low) {
        if (start >= end) return null;
        return next(text, start, end, low, contextAt(text, start, low));
    }

    /** Stateful path: classification depends only on the current token and saved context. */
    static Token next(CharSequence text, int start, int end, boolean low, int state) {
        if (start >= end) return null;
        int offset = start + 1;
        char first = text.charAt(start);
        if (Character.isWhitespace(first)) {
            while (offset < end && Character.isWhitespace(text.charAt(offset))) offset++;
            return new Token(Kind.SPACE, start, offset);
        }
        if (first == '#') {
            while (offset < end && text.charAt(offset) != '\n' && text.charAt(offset) != '\r') offset++;
            return new Token(Kind.COMMENT, start, offset);
        }
        if (first == '\'' || first == '"') {
            while (offset < end) {
                char current = text.charAt(offset);
                if (current == '\n' || current == '\r') break;
                offset++;
                if (current == '\\' && offset < end && text.charAt(offset) != '\n' && text.charAt(offset) != '\r') offset++;
                else if (current == first) break;
            }
            return new Token(Kind.STRING, start, offset);
        }
        if (first >= '0' && first <= '9') {
            while (offset < end && (isDigit(text.charAt(offset)) || text.charAt(offset) == '_')) offset++;
            if (offset + 1 < end && text.charAt(offset) == '.' && isDigit(text.charAt(offset + 1))) {
                offset++;
                while (offset < end && (isDigit(text.charAt(offset)) || text.charAt(offset) == '_')) offset++;
            }
            return new Token(Kind.NUMBER, start, offset);
        }
        if (isLetter(first) || first == '_') {
            while (offset < end && (isLetter(text.charAt(offset)) || isDigit(text.charAt(offset)) || text.charAt(offset) == '_')) offset++;
            String word = text.subSequence(start, offset).toString();
            Kind kind = COMMON.contains(word) || (low ? LOW : HIGH).contains(word)
                    || !low && word.equals("elif") && highElifHeader(text, start, offset, end) ? Kind.KEYWORD
                    : TYPES.contains(word) || state == TYPE_NAME ? Kind.TYPE : Kind.IDENTIFIER;
            return new Token(kind, start, offset);
        }
        Kind kind = switch (first) {
            case '(' -> Kind.LPAREN; case ')' -> Kind.RPAREN;
            case '[' -> Kind.LBRACKET; case ']' -> Kind.RBRACKET;
            case '{' -> low ? Kind.LBRACE : Kind.BAD; case '}' -> low ? Kind.RBRACE : Kind.BAD;
            case '+', '-', '*', '/', '%', '=', '<', '>', '!', '?' -> Kind.OPERATOR;
            case ':', ';', ',', '.', '@' -> Kind.PUNCTUATION;
            default -> Kind.BAD;
        };
        if (kind == Kind.OPERATOR && offset < end) {
            String pair = text.subSequence(start, offset + 1).toString();
            if (OPERATORS.contains(pair)) offset++;
        }
        return new Token(kind, start, offset);
    }
    /** Context transition shared by the editor lexer; no lookbehind or restart history is needed. */
    static int followingState(CharSequence text, Token token, int state, boolean low) {
        if (token.kind() == Kind.SPACE) {
            if (state != TYPE_NAME || token.end() - token.start() > MAX_DECLARATION_GAP) return CODE;
            for (int index = token.start(); index < token.end(); index++) {
                if (!horizontalSpace(text.charAt(index))) return CODE;
            }
            return TYPE_NAME;
        }
        if (token.kind() == Kind.KEYWORD && state != ADJACENT_PREFIX
                && typeDeclarationKeyword(text.subSequence(token.start(), token.end()).toString(), low)) return TYPE_NAME;
        return switch (token.kind()) {
            // These complete word tokens consume all adjacent identifier parts.
            // A subsequent keyword can only follow a separator, so CODE is a
            // safe restart boundary, including in a long dotted identifier chain.
            case IDENTIFIER, KEYWORD, TYPE, COMMENT, STRING -> CODE;
            // Numeric/punctuation tokens may end immediately before a keyword
            // (1class, 1_class, .class). Preserve the same prefix predicate used
            // by the bounded context restoration for stateless scanner clients.
            default -> blocksDeclarationPrefix(text.charAt(token.end() - 1)) ? ADJACENT_PREFIX : CODE;
        };
    }

    /** Restore only the bounded context required by stateless scanner clients. */
    private static int contextAt(CharSequence text, int start, boolean low) {
        if (start == 0) return CODE;
        int end = start;
        while (end > 0 && horizontalSpace(text.charAt(end - 1))) {
            if (start - end >= MAX_DECLARATION_GAP) return CODE;
            end--;
        }
        if (end == start) return blocksDeclarationPrefix(text.charAt(start - 1)) ? ADJACENT_PREFIX : CODE;
        int begin = end;
        while (begin > 0 && isLetter(text.charAt(begin - 1))) {
            if (end - begin > 6) return CODE;
            begin--;
        }
        if (begin > 0 && blocksDeclarationPrefix(text.charAt(begin - 1))) return CODE;
        return typeDeclarationKeyword(text.subSequence(begin, end).toString(), low) ? TYPE_NAME : CODE;
    }
    private static boolean horizontalSpace(char value) { return value == ' ' || value == '\t'; }
    private static boolean blocksDeclarationPrefix(char value) {
        return isLetter(value) || isDigit(value) || value == '_' || value == '.';
    }
    private static boolean typeDeclarationKeyword(String word, boolean low) {
        return word.equals("enum") || word.equals(low ? "record" : "class");
    }
    /** Match the same contextual shape as the High parser without stealing an existing identifier. */
    private static boolean highElifHeader(CharSequence text, int start, int wordEnd, int end) {
        int lineStart = start;
        while (lineStart > 0 && text.charAt(lineStart - 1) != '\n' && text.charAt(lineStart - 1) != '\r') lineStart--;
        for (int index = lineStart; index < start; index++) if (!horizontalSpace(text.charAt(index))) return false;
        int depth = 0;
        boolean condition = false;
        char quote = 0;
        for (int index = wordEnd; index < end; index++) {
            char value = text.charAt(index);
            if (value == '\n' || value == '\r') {
                if (depth > 0) continue;
                return false;
            }
            if (quote != 0) {
                if (value == '\\' && index + 1 < end) index++;
                else if (value == quote) quote = 0;
                condition = true;
                continue;
            }
            if (value == '#') return false;
            if (value == '\'' || value == '"') { quote = value; condition = true; continue; }
            if (value == '(' || value == '[') { depth++; condition = true; continue; }
            if (value == ')' || value == ']') { depth = Math.max(0, depth - 1); condition = true; continue; }
            if (value == ';' && depth == 0) return false;
            if (value == ':' && depth == 0) {
                if (!condition) return false;
                for (int tail = index + 1; tail < end; tail++) {
                    char rest = text.charAt(tail);
                    if (rest == '\n' || rest == '\r') return true;
                    if (rest == '#') return true;
                    if (!horizontalSpace(rest)) return false;
                }
                return true;
            }
            if (!Character.isWhitespace(value)) condition = true;
        }
        return false;
    }
    private static boolean isDigit(char value) { return value >= '0' && value <= '9'; }
    private static boolean isLetter(char value) { return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'; }
}
