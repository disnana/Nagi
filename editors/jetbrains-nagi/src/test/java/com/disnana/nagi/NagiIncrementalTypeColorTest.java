package com.disnana.nagi;

import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.ex.EditorEx;
import com.intellij.openapi.editor.ex.util.LexerEditorHighlighter;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.List;

/** Document edits exercise the platform's incremental lexer convergence, without PSI reparsing. */
public final class NagiIncrementalTypeColorTest extends BasePlatformTestCase {
    private boolean low;
    private int files;

    private void open(boolean low, String source) throws Exception {
        this.low = low;
        var file = myFixture.getTempDirFixture().createFile("incremental-" + files++ + (low ? ".low" : ".nagi"), source);
        assertEquals(source, new String(file.contentsToByteArray(), java.nio.charset.StandardCharsets.UTF_8));
        myFixture.configureFromExistingVirtualFile(file);
        // The platform loads CRLF bytes into an LF-only Document. Edits must
        // use that documented representation; the lexer restart test also
        // exercises a raw CRLF CharSequence directly.
        assertFalse(myFixture.getEditor().getDocument().getText().contains("\r"));
        matchesFresh();
    }
    private void replace(int start, int end, String replacement) {
        WriteCommandAction.runWriteCommandAction(getProject(),
                () -> myFixture.getEditor().getDocument().replaceString(start, end, replacement));
        matchesFresh();
    }
    private void point(NagiTokenScanner.Kind expected) {
        var editor = (EditorEx) myFixture.getEditor();
        int offset = editor.getDocument().getText().indexOf("Point");
        assertTrue(offset >= 0);
        assertSame(editor.getDocument().getText(), NagiTokens.get(expected),
                editor.getHighlighter().createIterator(offset).getTokenType());
    }
    private void matchesFresh() {
        var editor = (EditorEx) myFixture.getEditor();
        var fresh = new LexerEditorHighlighter(new NagiSyntaxHighlighter(low), editor.getColorsScheme());
        fresh.setText(editor.getDocument().getCharsSequence());
        var actual = editor.getHighlighter().createIterator(0);
        var expected = fresh.createIterator(0);
        while (!expected.atEnd()) {
            assertFalse(editor.getDocument().getText(), actual.atEnd());
            assertEquals(expected.getStart(), actual.getStart());
            assertEquals(expected.getEnd(), actual.getEnd());
            assertSame(editor.getDocument().getText(), expected.getTokenType(), actual.getTokenType());
            actual.advance(); expected.advance();
        }
        assertTrue(actual.atEnd());
    }
    private List<String> keywords(boolean low) { return List.of(low ? "record" : "class", "enum"); }

    public void testSameLengthKeywordEditsRecolorNamesAcrossSpacesTabsAndLineEndings() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            for (String gap : List.of(" ", "    ", "\t", " \t ")) for (String newline : List.of("\n", "\r\n")) {
                String other = keyword.substring(0, keyword.length() - 1) + "x";
                open(low, keyword + gap + "Point" + newline);
                point(NagiTokenScanner.Kind.TYPE);
                replace(keyword.length() - 1, keyword.length(), "x");
                point(NagiTokenScanner.Kind.IDENTIFIER);
                replace(0, other.length(), keyword);
                point(NagiTokenScanner.Kind.TYPE);
            }
        }
    }
    public void testKeywordInsertionDeletionAndIncompleteTypingRecolorTheExistingName() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            open(low, "Point\n");
            point(NagiTokenScanner.Kind.IDENTIFIER);
            replace(0, 0, keyword + " \t ");
            point(NagiTokenScanner.Kind.TYPE);
            replace(0, keyword.length() + 3, "");
            point(NagiTokenScanner.Kind.IDENTIFIER);
            replace(0, 0, keyword + " ");
            replace(keyword.length() - 1, keyword.length(), "");
            point(NagiTokenScanner.Kind.IDENTIFIER);
            replace(keyword.length() - 1, keyword.length() - 1, keyword.substring(keyword.length() - 1));
            point(NagiTokenScanner.Kind.TYPE);
            replace(keyword.length(), keyword.length() + 1, "");
            point(NagiTokenScanner.Kind.IDENTIFIER); // joined keyword/name token
            replace(keyword.length(), keyword.length(), "\t");
            point(NagiTokenScanner.Kind.TYPE);
        }
    }
    public void testCommentStringAndPhysicalLineBoundariesResetDeclarationState() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            for (String newline : List.of("\n", "\r\n")) {
                open(low, keyword + " Point" + newline + "Point" + newline);
                point(NagiTokenScanner.Kind.TYPE);
                replace(0, 0, "# ");
                point(NagiTokenScanner.Kind.COMMENT);
                replace(0, 2, "");
                point(NagiTokenScanner.Kind.TYPE);
                replace(0, 0, "\"");
                point(NagiTokenScanner.Kind.STRING);
                replace(0, 1, "");
                point(NagiTokenScanner.Kind.TYPE);
                replace(keyword.length(), keyword.length(), "\"");
                replace(0, 0, "\"");
                point(NagiTokenScanner.Kind.IDENTIFIER); // "class" Point
                replace(keyword.length() + 1, keyword.length() + 2, "");
                point(NagiTokenScanner.Kind.STRING); // unfinished quote ends at the physical line
                replace(0, 1, "");
                point(NagiTokenScanner.Kind.TYPE);
                replace(keyword.length() + 1, keyword.length() + 1, "\n");
                point(NagiTokenScanner.Kind.IDENTIFIER);
                replace(keyword.length() + 1, keyword.length() + 2, "");
                point(NagiTokenScanner.Kind.TYPE);
            }
        }
    }
    public void testChangingTheKeywordQualifierAndForeignLanguageKeywordsMatchesFreshLexing() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            open(low, "item." + keyword + " Point\n");
            point(NagiTokenScanner.Kind.IDENTIFIER);
            replace(4, 5, " ");
            point(NagiTokenScanner.Kind.TYPE);
            replace(4, 5, ".");
            point(NagiTokenScanner.Kind.IDENTIFIER);
        }
        open(false, "record Point\n");
        point(NagiTokenScanner.Kind.IDENTIFIER);
        open(true, "class Point\n");
        point(NagiTokenScanner.Kind.IDENTIFIER);
    }
    public void testHorizontalWhitespaceBudgetRemainsAnIncrementalBoundary() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            open(low, keyword + " ".repeat(NagiTokenScanner.MAX_DECLARATION_GAP) + "Point\n");
            point(NagiTokenScanner.Kind.TYPE);
            replace(keyword.length(), keyword.length(), " ");
            point(NagiTokenScanner.Kind.IDENTIFIER);
            replace(keyword.length(), keyword.length() + 1, "");
            point(NagiTokenScanner.Kind.TYPE);
        }
    }
    public void testAdjacentPrefixEditsRecolorAllDeclarationKeywords() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            for (String blocked : List.of("1", "1_", "1.2", "a", "_", ".", "enum")) {
                open(low, ";" + keyword + " Point\n");
                point(NagiTokenScanner.Kind.TYPE);
                replace(0, 1, blocked);
                point(NagiTokenScanner.Kind.IDENTIFIER);
                replace(0, blocked.length(), ";");
                point(NagiTokenScanner.Kind.TYPE);
            }
            open(low, "enum" + keyword + " Point\n");
            point(NagiTokenScanner.Kind.IDENTIFIER);
            replace(4, 4, " ");
            point(NagiTokenScanner.Kind.TYPE);
            replace(4, 5, "");
            point(NagiTokenScanner.Kind.IDENTIFIER);
        }
    }
    private record Lexed(int start, int end, int state, com.intellij.psi.tree.IElementType type) {}
    private List<Lexed> lex(String source, boolean low, int start, int state) {
        var result = new java.util.ArrayList<Lexed>();
        var lexer = new NagiLexer(low);
        lexer.start(source, start, source.length(), state);
        while (lexer.getTokenType() != null) {
            result.add(new Lexed(lexer.getTokenStart(), lexer.getTokenEnd(), lexer.getState(), lexer.getTokenType()));
            lexer.advance();
        }
        return result;
    }
    public void testFullLexStatelessScanAndEveryTokenRestartHaveEquivalentClassification() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            Integer allowed = null;
            for (String prefix : List.of(";", "1", "1_", "1.2", "a", "_", ".", "enum", "\"quoted\"", "# comment\n")) {
                for (String gap : List.of(" ", "\t", " \t ", "\n")) for (String newline : List.of("\n", "\r\n")) {
                    String source = prefix + keyword + gap + "Point" + newline + "Point";
                    var full = lex(source, low, 0, 0);
                    int point = source.indexOf("Point");
                    for (int index = 0; index < full.size(); index++) {
                        var token = full.get(index);
                        assertEquals(source, full.subList(index, full.size()), lex(source, low, token.start(), token.state()));
                        var scanned = NagiTokenScanner.next(source, token.start(), source.length(), low);
                        assertSame(source, token.type(), NagiTokens.get(scanned.kind()));
                        if (gap.equals(" ") && newline.equals("\n") && token.start() == point) {
                            if (prefix.equals(";")) allowed = token.state();
                            if (List.of("1", "1_", "1.2", "a", "_", ".", "enum").contains(prefix)) {
                                assertNotNull(allowed);
                                assertFalse("blocking prefix state must differ: " + source, allowed.equals(token.state()));
                            }
                        }
                    }
                }
            }
        }
    }
    public void testLexerRestartStateCarriesTheDeclarationExpectationAcrossWhitespace() throws Exception {
        for (boolean low : new boolean[] {false, true}) for (String keyword : keywords(low)) {
            String source = keyword + " \t Point\r\nPoint";
            var lexer = new NagiLexer(low);
            lexer.start(source, 0, source.length(), 0);
            assertEquals(0, lexer.getState());
            lexer.advance();
            int pending = lexer.getState();
            assertTrue(pending != 0);
            var fromSpace = new NagiLexer(low);
            fromSpace.start(source, lexer.getTokenStart(), source.length(), pending);
            assertEquals(pending, fromSpace.getState());
            fromSpace.advance();
            assertEquals(pending, fromSpace.getState());
            assertSame(NagiTokens.get(NagiTokenScanner.Kind.TYPE), fromSpace.getTokenType());
            lexer.advance();
            assertEquals(pending, lexer.getState());
            int name = lexer.getTokenStart();
            assertSame(NagiTokens.get(NagiTokenScanner.Kind.TYPE), lexer.getTokenType());
            var restarted = new NagiLexer(low);
            restarted.start(source, name, source.length(), pending);
            while (lexer.getTokenType() != null) {
                assertEquals(lexer.getState(), restarted.getState());
                assertSame(lexer.getTokenType(), restarted.getTokenType());
                assertEquals(lexer.getTokenStart(), restarted.getTokenStart());
                assertEquals(lexer.getTokenEnd(), restarted.getTokenEnd());
                lexer.advance(); restarted.advance();
            }
            assertNull(restarted.getTokenType());
        }
    }
}
