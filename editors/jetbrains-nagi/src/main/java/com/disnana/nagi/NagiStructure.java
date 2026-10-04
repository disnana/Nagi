package com.disnana.nagi;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

public final class NagiStructure {
    public record Range(int start, int end) {}
    private NagiStructure() {}

    public static List<Range> folds(CharSequence text, boolean low) {
        if (!low) return indentationFolds(text.toString());
        var ranges = new ArrayList<Range>();
        var stack = new ArrayDeque<NagiTokenScanner.Token>();
        for (var token = NagiTokenScanner.next(text, 0, text.length(), true); token != null;
             token = NagiTokenScanner.next(text, token.end(), text.length(), true)) {
            var kind = token.kind();
            if (kind == NagiTokenScanner.Kind.LPAREN || kind == NagiTokenScanner.Kind.LBRACKET || kind == NagiTokenScanner.Kind.LBRACE) stack.push(token);
            else if (kind == NagiTokenScanner.Kind.RPAREN || kind == NagiTokenScanner.Kind.RBRACKET || kind == NagiTokenScanner.Kind.RBRACE) {
                if (stack.isEmpty()) continue;
                var opening = stack.pop();
                if (!matches(opening.kind(), kind)) { stack.clear(); continue; }
                if (opening.kind() == NagiTokenScanner.Kind.LBRACE && containsNewline(text, opening.end(), token.start())) {
                    ranges.add(new Range(opening.end(), token.start()));
                }
            }
        }
        return ranges;
    }

    private static boolean matches(NagiTokenScanner.Kind open, NagiTokenScanner.Kind close) {
        return open == NagiTokenScanner.Kind.LPAREN && close == NagiTokenScanner.Kind.RPAREN
                || open == NagiTokenScanner.Kind.LBRACKET && close == NagiTokenScanner.Kind.RBRACKET
                || open == NagiTokenScanner.Kind.LBRACE && close == NagiTokenScanner.Kind.RBRACE;
    }
    private static boolean containsNewline(CharSequence text, int start, int end) {
        for (int index = start; index < end; index++) if (text.charAt(index) == '\n') return true;
        return false;
    }
    private static List<Range> indentationFolds(String text) {
        String[] lines = text.split("\n", -1);
        int[] offsets = new int[lines.length];
        int[] depths = new int[lines.length];
        for (int index = 1; index < lines.length; index++) offsets[index] = offsets[index - 1] + lines[index - 1].length() + 1;
        int depth = 0;
        for (int index = 0; index < lines.length; index++) {
            depths[index] = depth;
            String code = codeWithoutComments(lines[index], false);
            for (int character = 0; character < code.length(); character++) {
                char value = code.charAt(character);
                if (value == '(' || value == '[') depth++;
                else if (value == ')' || value == ']') depth = Math.max(0, depth - 1);
            }
        }
        var ranges = new ArrayList<Range>();
        for (int index = 0; index < lines.length - 1; index++) {
            String code = codeWithoutComments(lines[index], false).stripTrailing();
            if (!code.endsWith(":")) continue;
            int indentation = leadingWhitespace(lines[index]).length();
            int last = index;
            for (int next = index + 1; next < lines.length; next++) {
                if (codeWithoutComments(lines[next], false).isBlank()) continue;
                if (depths[next] == 0 && leadingWhitespace(lines[next]).length() <= indentation) break;
                last = next;
            }
            if (last > index) ranges.add(new Range(offsets[index] + code.length(), offsets[last] + lines[last].stripTrailing().length()));
        }
        return ranges;
    }

    /** Spaces only for new levels; existing indentation is retained. No whole-file formatting. */
    public static String indentAfter(String line, boolean low, int indentSize) {
        String base = leadingWhitespace(line);
        String code = codeWithoutComments(line, low).stripTrailing();
        int nesting = 0;
        for (var token = NagiTokenScanner.next(code, 0, code.length(), low); token != null;
             token = NagiTokenScanner.next(code, token.end(), code.length(), low)) {
            switch (token.kind()) {
                case LPAREN, LBRACKET -> nesting++;
                case RPAREN, RBRACKET -> nesting--;
                case LBRACE -> { if (low) nesting++; }
                case RBRACE -> { if (low) nesting--; }
                default -> {}
            }
        }
        boolean block = !low && code.endsWith(":");
        return base + (block || nesting > 0 ? " ".repeat(Math.max(1, indentSize)) : "");
    }
    private record Opening(NagiTokenScanner.Kind kind, int lineStart) {}

    public static String indentAt(CharSequence prefix, boolean low, int indentSize) {
        String text = prefix.toString();
        int currentStart = text.lastIndexOf('\n') + 1;
        String current = text.substring(currentStart);
        String base = leadingWhitespace(current);
        var stack = new ArrayDeque<Opening>();
        Opening closed = null;
        int lineStart = 0;
        for (var token = NagiTokenScanner.next(text, 0, text.length(), low); token != null;
             token = NagiTokenScanner.next(text, token.end(), text.length(), low)) {
            for (int index = token.start(); index < token.end(); index++) if (text.charAt(index) == '\n') lineStart = index + 1;
            switch (token.kind()) {
                case LPAREN, LBRACKET, LBRACE -> stack.push(new Opening(token.kind(), lineStart));
                case RPAREN, RBRACKET, RBRACE -> {
                    if (stack.isEmpty()) break;
                    var opening = stack.pop();
                    if (!matches(opening.kind(), token.kind())) { stack.clear(); closed = null; break; }
                    if (lineStart == currentStart && opening.lineStart() < currentStart) closed = opening;
                }
                default -> {}
            }
        }
        if (!stack.isEmpty()) {
            Opening opening = stack.peek();
            int end = text.indexOf('\n', opening.lineStart());
            String anchor = text.substring(opening.lineStart(), end == -1 ? text.length() : end);
            return leadingWhitespace(anchor) + " ".repeat(Math.max(1, indentSize));
        }
        if (closed != null) {
            int end = text.indexOf('\n', closed.lineStart());
            String anchor = text.substring(closed.lineStart(), end == -1 ? text.length() : end);
            base = leadingWhitespace(anchor);
        }
        String code = codeWithoutComments(current, low).stripTrailing();
        return base + (!low && code.endsWith(":") ? " ".repeat(Math.max(1, indentSize)) : "");
    }
    public static String leadingWhitespace(String line) {
        int end = 0;
        while (end < line.length() && (line.charAt(end) == ' ' || line.charAt(end) == '\t')) end++;
        return line.substring(0, end);
    }
    private static String codeWithoutComments(String line, boolean low) {
        StringBuilder code = new StringBuilder(line);
        for (var token = NagiTokenScanner.next(line, 0, line.length(), low); token != null;
             token = NagiTokenScanner.next(line, token.end(), line.length(), low)) {
            if (token.kind() == NagiTokenScanner.Kind.COMMENT || token.kind() == NagiTokenScanner.Kind.STRING) {
                for (int index = token.start(); index < token.end(); index++) code.setCharAt(index, ' ');
            }
        }
        return code.toString();
    }
}
