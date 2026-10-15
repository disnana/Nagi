package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.editorActions.SmartBackspaceMode;
import com.intellij.codeInsight.lookup.Lookup;
import com.intellij.codeInsight.lookup.LookupElement;
import com.intellij.openapi.actionSystem.IdeActions;
import com.intellij.psi.codeStyle.CodeStyleSettingsManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.Arrays;
import java.util.function.LongSupplier;

/** Real platform lookup and Backspace actions; no compiler process is needed for these cases. */
public final class NagiFeedbackEditorTest extends BasePlatformTestCase {
    public void testBackspaceModesAndConfiguredWidths() {
        var settings = CodeInsightSettings.getInstance();
        SmartBackspaceMode previousMode = settings.getBackspaceMode();
        try {
            var override = new NagiBackspaceModeOverride();
            var modeFile = myFixture.configureByText("mode.nagi", "<caret>");
            assertEquals(SmartBackspaceMode.INDENT, override.getBackspaceMode(modeFile, SmartBackspaceMode.AUTOINDENT));
            assertEquals(SmartBackspaceMode.OFF, override.getBackspaceMode(modeFile, SmartBackspaceMode.OFF));
            assertEquals(SmartBackspaceMode.INDENT, override.getBackspaceMode(modeFile, SmartBackspaceMode.INDENT));
            settings.setBackspaceMode(SmartBackspaceMode.AUTOINDENT);
            for (var language : new NagiLanguage[] { NagiLanguage.HIGH, NagiLanguage.LOW }) {
                for (int width : new int[] { 2, 4, 8 }) {
                    withWidth(language, width, () -> {
                        String extension = language == NagiLanguage.HIGH ? ".nagi" : ".low";
                        String first = language == NagiLanguage.HIGH ? "def main():" : "fn main() {";
                        String body = language == NagiLanguage.HIGH ? "value = 1" : "let value = 1;";
                        String text = first + "\n" + " ".repeat(width * 2) + "<caret>" + body;
                        var file = myFixture.configureByText("backspace-" + width + extension, text);
                        assertEquals(width, CodeStyle.getIndentOptions(file).INDENT_SIZE);
                        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
                        myFixture.checkResult(first + "\n" + " ".repeat(width) + "<caret>" + body);
                        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
                        myFixture.checkResult(first + "\n<caret>" + body);
                    });
                }
            }
        } finally { settings.setBackspaceMode(previousMode); }
    }

    public void testOffLeavesSingleCharacterDeletionAndBodyTextUntouched() {
        var settings = CodeInsightSettings.getInstance();
        SmartBackspaceMode previousMode = settings.getBackspaceMode();
        try {
            settings.setBackspaceMode(SmartBackspaceMode.OFF);
            myFixture.configureByText("off.nagi", "def main():\n    <caret>value = 1");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            myFixture.checkResult("def main():\n   <caret>value = 1");
            settings.setBackspaceMode(SmartBackspaceMode.AUTOINDENT);
            myFixture.configureByText("code.nagi", "def main():\n    ca<caret>");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            myFixture.checkResult("def main():\n    c<caret>");
            myFixture.configureByText("comment.nagi", "def main():\n    # <caret>note");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            myFixture.checkResult("def main():\n    #<caret>note");
        } finally { settings.setBackspaceMode(previousMode); }
    }

    public void testSelectionPairsMultipleCaretsAndUndoRedoStayPlatformOwned() {
        var settings = CodeInsightSettings.getInstance();
        SmartBackspaceMode previousMode = settings.getBackspaceMode();
        try {
            settings.setBackspaceMode(SmartBackspaceMode.AUTOINDENT);
            myFixture.configureByText("selected.nagi", "def main():\n    <selection>value</selection>");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            assertEquals("def main():\n    ", myFixture.getEditor().getDocument().getText());
            myFixture.configureByText("quote.nagi", "value = <caret>");
            myFixture.type('\'');
            myFixture.checkResult("value = '<caret>'");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            myFixture.checkResult("value = <caret>");
            myFixture.configureByText("brace.low", "fn main() <caret>");
            myFixture.type('{');
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            myFixture.checkResult("fn main() <caret>");

            myFixture.configureByText("multi.nagi", "def main():\n    <caret>first = 1\n    second = 2");
            var editor = myFixture.getEditor();
            int second = editor.getDocument().getText().indexOf("    second") + 4;
            assertNotNull(editor.getCaretModel().addCaret(editor.offsetToVisualPosition(second)));
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
            assertEquals("def main():\nfirst = 1\nsecond = 2", editor.getDocument().getText());
            myFixture.performEditorAction(IdeActions.ACTION_UNDO);
            assertEquals("def main():\n    first = 1\n    second = 2", editor.getDocument().getText());
            myFixture.performEditorAction(IdeActions.ACTION_REDO);
            assertEquals("def main():\nfirst = 1\nsecond = 2", editor.getDocument().getText());
        } finally { settings.setBackspaceMode(previousMode); }
    }

    public void testLexicalDeclarationLookupHasNarrowHighLowContexts() {
        assertContains("declaration.nagi", "d<caret>", "def");
        assertContains("continued.nagi", "de<caret>", "def");
        assertContains("declaration.low", "f<caret>", "fn");
        for (String text : new String[] { "def main():\n    d<caret>", "class Box:\n    d<caret>",
                "# d<caret>", "\"d<caret>\"", "user.d<caret>", "d<caret>ef" }) {
            assertMissing("not-declaration.nagi", text, "def");
        }
        assertMissing("not-declaration.low", "fn main() { f<caret> }", "fn");
        assertMissing("wrong-language.low", "d<caret>", "def");
        assertMissing("wrong-language.nagi", "f<caret>", "fn");
        assertMissing("plain.txt", "d<caret>", "def");
    }

    public void testDeclarationKeywordDoesNotLeakAcrossEarlierLineContinuationOrBlock() {
        assertMissing("block.low", "fn main() {\nf<caret>\n}", "fn");
        assertMissing("continuation.nagi", "values = (\nd<caret>\n)", "def");
    }

    public void testDeclarationKeywordRequiresCertainLexicalContext() {
        assertContains("after-string.nagi", "message = \"( and \\\" )\"\nd<caret>", "def");
        assertContains("after-comment.nagi", "# ( {\nd<caret>", "def");
        assertContains("after-balanced.nagi", "values = (1, 2)\nd<caret>", "def");
        assertContains("after-block.low", "fn main() {\n}\nf<caret>", "fn");
        assertMissing("unfinished-string.nagi", "message = \"unfinished\nd<caret>", "def");
        assertMissing("unfinished-escape.nagi", "message = \"escaped \\\"\nd<caret>", "def");
        assertMissing("raw-like.nagi", "message = r\"(\"\nd<caret>", "def");
        assertMissing("unicode-unknown.nagi", "π = 1\nd<caret>", "def");
        assertMissing("crlf-quote.nagi", "message = \"unfinished\r\nd<caret>", "def");
        assertMissing("crlf-continuation.nagi", "values = (\r\nd<caret>\r\n)", "def");
        assertMissing("mismatched.low", "fn main() {\n]\nf<caret>", "fn");
    }

    public void testDeclarationContextBoundsAndDeadlineAreDeterministic() {
        assertFalse(NagiDeclarationContext.isCertainTopLevel(
                " ".repeat(NagiDeclarationContext.MAX_PREFIX_BYTES) + "d",
                NagiDeclarationContext.MAX_PREFIX_BYTES + 1, false));
        String widePrefix = "\"" + "é".repeat(4096) + "\"\nd";
        assertFalse(NagiDeclarationContext.isCertainTopLevel(widePrefix, widePrefix.length(), false));
        String byteBoundary = "#" + "a".repeat(8189) + "\nd";
        assertTrue(NagiDeclarationContext.isCertainTopLevel(byteBoundary, byteBoundary.length(), false));
        String manyTokens = "x ".repeat(NagiDeclarationContext.MAX_TOKENS) + "d";
        assertFalse(NagiDeclarationContext.isCertainTopLevel(manyTokens, manyTokens.length(), false));
        LongSupplier expired = new LongSupplier() {
            int reads;
            @Override public long getAsLong() { return reads++ == 0 ? 0 : 10_000_001L; }
        };
        String finitePrefix = "x ".repeat(40) + "d";
        assertFalse(NagiDeclarationContext.isCertainTopLevel(
                finitePrefix, finitePrefix.length(), false, expired));
        assertTrue(NagiDeclarationContext.isCertainTopLevel("values = (1, 2)\nd", 17, false));
    }

    public void testKeywordAcceptanceDoesNotExpandLiveTemplate() {
        for (String extension : new String[] { ".nagi", ".low" }) {
            String keyword = extension.equals(".nagi") ? "def" : "fn";
            myFixture.configureByText("insert" + extension, keyword.substring(0, 1) + "<caret>");
            LookupElement[] offered = myFixture.completeBasic();
            assertNotNull(offered);
            LookupElement candidate = Arrays.stream(offered)
                    .filter(item -> item.getLookupString().equals(keyword)).findFirst().orElse(null);
            assertNotNull(candidate);
            assertNotNull(myFixture.getLookup());
            myFixture.getLookup().setCurrentItem(candidate);
            myFixture.finishLookup(Lookup.NORMAL_SELECT_CHAR);
            assertEquals(keyword, myFixture.getEditor().getDocument().getText());
        }
    }

    private void assertContains(String name, String text, String keyword) {
        myFixture.configureByText(name, text);
        LookupElement[] offered = myFixture.completeBasic();
        assertNotNull(offered);
        assertTrue(Arrays.stream(offered).anyMatch(item -> item.getLookupString().equals(keyword)));
    }

    private void assertMissing(String name, String text, String keyword) {
        myFixture.configureByText(name, text);
        LookupElement[] offered = myFixture.completeBasic();
        assertTrue(offered == null || Arrays.stream(offered)
                .noneMatch(item -> item.getLookupString().equals(keyword)));
    }

    private void withWidth(NagiLanguage language, int width, Runnable test) {
        var previous = CodeStyleSettingsManager.getInstance(getProject()).getTemporarySettings();
        var style = CodeStyle.getSettings(getProject()).clone();
        style.getLanguageIndentOptions(language).INDENT_SIZE = width;
        CodeStyle.setTemporarySettings(getProject(), style);
        try { test.run(); }
        finally {
            if (previous == null) CodeStyle.dropTemporarySettings(getProject());
            else CodeStyle.setTemporarySettings(getProject(), previous);
        }
    }
}
