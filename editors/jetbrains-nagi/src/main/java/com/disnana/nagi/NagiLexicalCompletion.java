package com.disnana.nagi;

import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.regex.Pattern;

/** Bounded lexical hints only. Never guesses a binding, receiver type or import export. */
final class NagiLexicalCompletion {
    private static final int MAX_LINE = 8192;
    private static final Set<String> COMMON = Set.of("import", "from", "as", "extern", "enum", "async", "await",
            "return", "match", "case", "if", "else", "while", "for", "in", "with", "scope", "spawn",
            "try", "and", "or", "not", "true", "false", "True", "False", "None", "null");
    static final List<String> TYPES = List.of("i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64",
            "f32", "f64", "bool", "str", "bytes", "unit", "List", "Result", "Error", "view", "shared",
            "UUID", "timestamp", "Option", "Map", "fn");
    private static final Pattern TYPE = Pattern.compile("(?:->|\\b[A-Za-z_]\\w*\\s*:)\\s*[\\w.\\[\\], ?]*$");
    private static final Pattern IMPORT = Pattern.compile("^\\s*(?:import|from)\\b");
    private static final Pattern DECLARATION_NAME = Pattern.compile("\\b(?:def|fn|class|record|enum|as)\\s+$");
    private NagiLexicalCompletion() {}

    static List<String> candidates(CharSequence text, int offset, boolean low) {
        if (offset < 1 || offset > text.length()) return List.of();
        int start = offset;
        while (start > 0 && word(text.charAt(start - 1))) {
            if (offset - start >= MAX_LINE) return List.of();
            start--;
        }
        if (start == offset || Character.isDigit(text.charAt(start))) return List.of();
        int lineStart = start;
        while (lineStart > 0 && text.charAt(lineStart - 1) != '\n' && text.charAt(lineStart - 1) != '\r') {
            if (start - lineStart >= MAX_LINE) return List.of();
            lineStart--;
        }
        // The same lexer masks comment/string punctuation, including a
        // member receiver separated from its suffix by comments or newlines.
        int scan = Math.max(0, start - MAX_LINE);
        if (scan > 0) {
            while (scan < start && text.charAt(scan - 1) != '\n') scan++;
        }
        NagiTokenScanner.Token previous = null;
        int statement = lineStart;
        for (var token = NagiTokenScanner.next(text, scan, start, low); token != null;
             token = NagiTokenScanner.next(text, token.end(), start, low)) {
            if (token.kind() == NagiTokenScanner.Kind.COMMENT || token.kind() == NagiTokenScanner.Kind.SPACE) continue;
            previous = token;
            if (low && token.start() >= lineStart && (token.kind() == NagiTokenScanner.Kind.RBRACE
                    || token.kind() == NagiTokenScanner.Kind.PUNCTUATION && text.charAt(token.start()) == ';')) {
                statement = token.end();
            }
        }
        if (previous == null && scan > 0 || previous != null && previous.kind() == NagiTokenScanner.Kind.PUNCTUATION
                && text.charAt(previous.start()) == '.') return List.of();
        String before = text.subSequence(statement, start).toString();
        if (IMPORT.matcher(before).find() || DECLARATION_NAME.matcher(before).find()) return List.of();
        List<String> result = new ArrayList<>();
        if (TYPE.matcher(before).find()) result.addAll(TYPES);
        else if (before.matches("(?s).*\\bcase\\s+$")) result.addAll(List.of("Ok", "Err", "Some", "None"));
        else {
            result.addAll(COMMON);
            if (low) result.add("let");
            // Declaration hints retain the existing conservative top-level guard.
            if (before.isBlank() && lineStart == start && NagiDeclarationContext.isCertainTopLevel(text, start, low)) {
                result.add(low ? "record" : "class");
            } else result.remove("enum");
        }
        String prefix = text.subSequence(start, offset).toString();
        return result.stream().filter(name -> name.startsWith(prefix)).sorted().toList();
    }
    private static boolean word(char value) {
        return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'
                || value >= '0' && value <= '9' || value == '_';
    }
}
