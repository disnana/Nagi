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
            "f32", "f64", "bool", "str", "bytes", "unit", "List", "Result", "Error", "view", "shared");
    private static final Set<String> OPERATORS = Set.of("->", "==", "!=", "<=", ">=", "+=", "-=", "*=");

    private NagiTokenScanner() {}

    public static Token next(CharSequence text, int start, int end, boolean low) {
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
            Kind kind = COMMON.contains(word) || (low ? LOW : HIGH).contains(word) ? Kind.KEYWORD
                    : TYPES.contains(word) ? Kind.TYPE : Kind.IDENTIFIER;
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
    private static boolean isDigit(char value) { return value >= '0' && value <= '9'; }
    private static boolean isLetter(char value) { return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'; }
}
