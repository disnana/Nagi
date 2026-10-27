package com.disnana.nagi;

import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.util.TextRange;
import com.intellij.psi.PsiFile;
import com.intellij.psi.codeStyle.CommonCodeStyleSettings;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;

/** Bounded lexical indentation corrections for typed branch and closing delimiters. */
final class NagiTypedIndent {
    private static final int MAX_CHARS = 8192;
    private static final int MAX_LINES = 96;
    private static final int MAX_TOKENS = 4096;
    private static final int MAX_COLUMNS = 8192;

    private NagiTypedIndent() {}

    static void alignTypedColon(PsiFile file, Editor editor, int colonOffset,
                                CommonCodeStyleSettings.IndentOptions options) {
        if (file.getLanguage() != NagiLanguage.HIGH) return;
        if (!validWidths(options)) return;
        Document document = editor.getDocument();
        int line = document.getLineNumber(colonOffset);
        int lineEnd = document.getLineEndOffset(line);
        if (lineEnd > MAX_CHARS || line + 1 > MAX_LINES) return;
        String text = document.getText(new TextRange(0, lineEnd));
        Lexed lexed = lex(text, false);
        if (lexed == null) return;

        Token typed = lexed.tokenAt(colonOffset);
        if (typed == null || typed.kind() != NagiTokenScanner.Kind.PUNCTUATION
                || !text.substring(typed.start(), typed.end()).equals(":")) return;

        BranchMatch match = findBranchMatch(lexed, line, colonOffset,
                Math.max(1, options.INDENT_SIZE), Math.max(1, options.TAB_SIZE));
        if (match == null) return;
        int delta = match.targetIndent() - match.statementIndent();
        if (delta == 0) return;

        List<IndentEdit> edits = new ArrayList<>();
        for (int current = match.startLine(); current <= line; current++) {
            int start = document.getLineStartOffset(current);
            int end = document.getLineEndOffset(current);
            String value = document.getText(new TextRange(start, end));
            if (value.isBlank()) continue;
            int oldLength = leadingWhitespaceLength(value);
            int oldColumns = visualWidth(value.substring(0, oldLength), options.TAB_SIZE);
            long wantedColumns = Math.max(0L, (long) oldColumns + delta);
            if (oldColumns > MAX_COLUMNS || wantedColumns > MAX_COLUMNS) return;
            int wanted = (int) wantedColumns;
            if (oldLength != wanted || !allSpaces(value, oldLength)) {
                edits.add(new IndentEdit(start, oldLength, " ".repeat(wanted)));
            }
        }
        applyEdits(document, editor, edits);
    }

    static void alignTypedCloser(PsiFile file, Editor editor, char closer, int closerOffset,
                                 CommonCodeStyleSettings.IndentOptions options) {
        boolean low = file.getLanguage() == NagiLanguage.LOW;
        if (!low && file.getLanguage() != NagiLanguage.HIGH) return;
        if (!validWidths(options)) return;
        Document document = editor.getDocument();
        if (closerOffset < 0 || closerOffset >= document.getTextLength()) return;
        int line = document.getLineNumber(closerOffset);
        if (closerOffset + 1 > MAX_CHARS || line + 1 > MAX_LINES) return;
        int lineStart = document.getLineStartOffset(line);
        if (!onlyHorizontalWhitespace(document.getCharsSequence(), lineStart, closerOffset)) return;
        String text = document.getText(new TextRange(0, closerOffset + 1));
        Lexed lexed = lex(text, low);
        if (lexed == null) return;
        Token typed = lexed.tokenAt(closerOffset);
        if (typed == null || tokenChar(typed.kind()) != closer) return;

        Deque<Opening> stack = new ArrayDeque<>();
        Opening found = null;
        for (Token token : lexed.tokens()) {
            if (token.start() > closerOffset) break;
            if (token.kind() == NagiTokenScanner.Kind.STRING || token.kind() == NagiTokenScanner.Kind.COMMENT) {
                if (token.start() <= closerOffset && closerOffset < token.end()) return;
                continue;
            }
            if (token.kind() == NagiTokenScanner.Kind.BAD) return;
            char value = tokenChar(token.kind());
            if (isOpening(value)) {
                int tokenLine = lineNumberAt(lexed.lines(), token.start());
                int tokenLineStart = lexed.lines().get(tokenLine).start();
                int indentEnd = firstNonWhitespace(text, tokenLineStart, lexed.lines().get(tokenLine).end());
                int indent = visualWidth(text.substring(tokenLineStart, indentEnd), options.TAB_SIZE);
                if (indent > MAX_COLUMNS) return;
                stack.push(new Opening(value, indent));
            } else if (isClosing(value)) {
                if (token.start() == closerOffset) {
                    if (stack.isEmpty() || matching(value) != stack.peek().value()) return;
                    found = stack.peek();
                }
                if (stack.isEmpty() || matching(value) != stack.peek().value()) return;
                stack.pop();
            }
        }
        if (found == null) return;

        // The guard above proved the entire prefix is horizontal whitespace;
        // do not copy a potentially unbounded comment/suffix after the closer.
        int oldLength = closerOffset - lineStart;
        int target = found.indent();
        if (target > MAX_COLUMNS) return;
        if (oldLength == target && allSpaces(document.getText(new TextRange(lineStart, lineStart + oldLength)), oldLength)) return;
        applyEdits(document, editor, List.of(new IndentEdit(lineStart, oldLength, " ".repeat(target))));
    }

    private static BranchMatch findBranchMatch(Lexed lexed, int targetLine, int colonOffset,
                                               int indentSize, int tabSize) {
        Deque<Character> delimiters = new ArrayDeque<>();
        List<Token> statement = new ArrayList<>();
        int statementStartLine = -1;
        int statementIndent = 0;
        Deque<Block> blocks = new ArrayDeque<>();

        for (int lineNumber = 0; lineNumber <= targetLine; lineNumber++) {
            Line line = lexed.lines().get(lineNumber);
            List<Token> code = new ArrayList<>();
            for (Token token : lexed.tokens()) {
                if (token.start() >= line.end()) break;
                if (token.start() >= line.start() && token.kind() != NagiTokenScanner.Kind.SPACE
                        && token.kind() != NagiTokenScanner.Kind.COMMENT) code.add(token);
            }
            if (code.isEmpty()) continue;
            if (delimiters.isEmpty() && statement.isEmpty()) {
                statementStartLine = lineNumber;
                statementIndent = visualWidth(lexed.text().substring(line.start(), firstNonWhitespace(
                        lexed.text(), line.start(), line.end())), tabSize);
                if (statementIndent > MAX_COLUMNS) return null;
            }
            for (Token token : code) {
                if (token.kind() == NagiTokenScanner.Kind.BAD) return null;
                if (lineNumber == targetLine && token.start() == colonOffset
                        && (token.kind() != NagiTokenScanner.Kind.PUNCTUATION
                        || !lexed.text().substring(token.start(), token.end()).equals(":"))) return null;
                statement.add(token);
                char value = tokenChar(token.kind());
                if (isOpening(value)) delimiters.push(value);
                else if (isClosing(value)) {
                    if (delimiters.isEmpty() || matching(value) != delimiters.peek()) return null;
                    delimiters.pop();
                }
            }
            if (delimiters.isEmpty()) {
                Header header = header(statement, lexed.text());
                if (lineNumber == targetLine) {
                    Token finalCodeToken = statement.get(statement.size() - 1);
                    if (finalCodeToken.start() != colonOffset || header == null
                            || header.branch() == Branch.NONE) return null;
                    Block parent = branchParent(blocks, header.branch(), statementIndent, indentSize);
                    if (parent == null) return null;
                    long targetIndent = (long) parent.indent()
                            + (header.branch() == Branch.CASE ? indentSize : 0);
                    if (targetIndent > MAX_COLUMNS) return null;
                    return new BranchMatch(statementStartLine, statementIndent,
                            (int) targetIndent);
                }

                if (header != null) {
                    // A recognized else/case consumes the blocks at its indentation,
                    // even when its owner is absent or too far away to align.
                    while (!blocks.isEmpty() && blocks.peek().indent() >= statementIndent) blocks.pop();
                    String blockKind = header.ownerKind();
                    if (blockKind == null) blockKind = header.branch() == Branch.ELSE ? "else" : "case";
                    blocks.push(new Block(blockKind, statementIndent));
                } else {
                    while (!blocks.isEmpty() && blocks.peek().indent() >= statementIndent) blocks.pop();
                }
                statement.clear();
                statementStartLine = -1;
            }
        }
        return null;
    }

    private static Header header(List<Token> statement, String text) {
        if (statement.isEmpty()) return null;
        Token first = statement.get(0);
        if (first.kind() != NagiTokenScanner.Kind.KEYWORD) return null;
        Token last = statement.get(statement.size() - 1);
        if (last.kind() != NagiTokenScanner.Kind.PUNCTUATION
                || !text.substring(last.start(), last.end()).equals(":")) return null;
        String word = text.substring(first.start(), first.end());
        return switch (word) {
            case "else" -> new Header(Branch.ELSE, null);
            // A bare `elif:` is also the start of an existing local annotation.
            case "elif" -> statement.size() > 2 ? new Header(Branch.ELSE, "if") : null;
            case "case" -> new Header(Branch.CASE, null);
            case "if", "match", "def", "class", "enum", "while", "for", "with", "scope", "try" ->
                    new Header(Branch.NONE, word);
            case "async" -> statement.size() > 1
                    && text.substring(statement.get(1).start(), statement.get(1).end()).equals("def")
                    ? new Header(Branch.NONE, "async") : null;
            default -> null;
        };
    }

    private static Block branchParent(Deque<Block> blocks, Branch branch, int indent, int indentSize) {
        String wanted = branch == Branch.ELSE ? "if" : "match";
        for (Block block : blocks) {
            int extra = branch == Branch.CASE ? indentSize : 0;
            if (block.kind().equals(wanted) && block.indent() + extra <= indent) return block;
        }
        return null;
    }

    private static Lexed lex(String text, boolean low) {
        if (text.length() > MAX_CHARS) return null;
        List<Line> lines = new ArrayList<>();
        List<Token> tokens = new ArrayList<>();
        int start = 0;
        while (start <= text.length()) {
            int end = start;
            while (end < text.length() && text.charAt(end) != '\n' && text.charAt(end) != '\r') end++;
            lines.add(new Line(start, end));
            int offset = start;
            while (offset < end) {
                NagiTokenScanner.Token scanned = NagiTokenScanner.next(text, offset, end, low);
                if (scanned == null || scanned.end() <= offset) return null;
                tokens.add(new Token(scanned.kind(), scanned.start(), scanned.end()));
                if (tokens.size() > MAX_TOKENS) return null;
                offset = scanned.end();
            }
            if (end == text.length()) break;
            if (text.charAt(end) == '\r' && end + 1 < text.length() && text.charAt(end + 1) == '\n') start = end + 2;
            else start = end + 1;
            if (lines.size() >= MAX_LINES && start <= text.length()) return null;
        }
        return new Lexed(text, lines, tokens);
    }

    private static int lineNumberAt(List<Line> lines, int offset) {
        for (int index = 0; index < lines.size(); index++) {
            Line line = lines.get(index);
            if (offset >= line.start() && offset <= line.end()) return index;
        }
        return Math.max(0, lines.size() - 1);
    }

    private static int firstNonWhitespace(String text, int start, int end) {
        int offset = start;
        while (offset < end && (text.charAt(offset) == ' ' || text.charAt(offset) == '\t')) offset++;
        return offset;
    }

    private static boolean validWidths(CommonCodeStyleSettings.IndentOptions options) {
        return options.INDENT_SIZE > 0 && options.INDENT_SIZE <= MAX_COLUMNS
                && options.TAB_SIZE > 0 && options.TAB_SIZE <= MAX_COLUMNS;
    }

    private static boolean onlyHorizontalWhitespace(CharSequence text, int start, int end) {
        for (int offset = start; offset < end; offset++) {
            char value = text.charAt(offset);
            if (value != ' ' && value != '\t') return false;
        }
        return true;
    }

    private static int leadingWhitespaceLength(String text) {
        int offset = 0;
        while (offset < text.length() && (text.charAt(offset) == ' ' || text.charAt(offset) == '\t')) offset++;
        return offset;
    }

    private static int visualWidth(String whitespace, int tabSize) {
        int width = 0;
        for (int index = 0; index < whitespace.length(); index++) {
            width = whitespace.charAt(index) == '\t' ? width + tabSize - width % tabSize : width + 1;
        }
        return width;
    }

    private static boolean allSpaces(String value, int length) {
        for (int index = 0; index < length; index++) if (value.charAt(index) != ' ') return false;
        return true;
    }

    private static void applyEdits(Document document, Editor editor, List<IndentEdit> edits) {
        if (edits.isEmpty()) return;
        int oldCaret = editor.getCaretModel().getOffset();
        int shift = 0;
        for (IndentEdit edit : edits) {
            if (edit.start() + edit.oldLength() <= oldCaret) shift += edit.replacement().length() - edit.oldLength();
        }
        List<IndentEdit> ordered = new ArrayList<>(edits);
        ordered.sort((left, right) -> Integer.compare(right.start(), left.start()));
        for (IndentEdit edit : ordered) {
            document.replaceString(edit.start(), edit.start() + edit.oldLength(), edit.replacement());
        }
        editor.getCaretModel().moveToOffset(Math.max(0, Math.min(document.getTextLength(), oldCaret + shift)));
    }

    private static char tokenChar(NagiTokenScanner.Kind kind) {
        return switch (kind) {
            case LPAREN -> '('; case RPAREN -> ')';
            case LBRACKET -> '['; case RBRACKET -> ']';
            case LBRACE -> '{'; case RBRACE -> '}';
            default -> '\0';
        };
    }

    private static boolean isOpening(char value) { return value == '(' || value == '[' || value == '{'; }
    private static boolean isClosing(char value) { return value == ')' || value == ']' || value == '}'; }
    private static char matching(char close) {
        return switch (close) { case ')' -> '('; case ']' -> '['; case '}' -> '{'; default -> '\0'; };
    }

    private enum Branch { NONE, ELSE, CASE }
    private record Block(String kind, int indent) {}
    private record Header(Branch branch, String ownerKind) {}
    private record BranchMatch(int startLine, int statementIndent, int targetIndent) {}
    private record Opening(char value, int indent) {}
    private record Line(int start, int end) {}
    private record Token(NagiTokenScanner.Kind kind, int start, int end) {}
    private record IndentEdit(int start, int oldLength, String replacement) {}
    private record Lexed(String text, List<Line> lines, List<Token> tokens) {
        private Token tokenAt(int offset) {
            for (Token token : tokens) if (offset >= token.start() && offset < token.end()) return token;
            return null;
        }
    }
}
