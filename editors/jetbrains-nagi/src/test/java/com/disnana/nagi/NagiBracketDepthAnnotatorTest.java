package com.disnana.nagi;

import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.impl.DocumentMarkupModel;
import com.intellij.openapi.editor.markup.RangeHighlighter;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.Arrays;
import java.util.List;
import java.util.Set;

public final class NagiBracketDepthAnnotatorTest extends BasePlatformTestCase {
    private boolean previous;

    @Override protected void setUp() throws Exception {
        super.setUp();
        previous = NagiSettings.getInstance().getState().bracketDepthColors;
        setEnabled(false);
    }
    @Override protected void tearDown() throws Exception {
        try { setEnabled(previous); }
        finally { super.tearDown(); }
    }

    public void testDisabledByDefaultAndEnabledPairsUseDepthKeys() {
        String text = "def main():\n    print((1 + [2])) # ([ignored])\n    text = \"([ignored])\"\n";
        myFixture.configureByText(NagiFileType.HIGH, text);
        myFixture.doHighlighting();
        assertTrue(depthHighlighters().isEmpty());

        setEnabled(true);
        myFixture.doHighlighting();
        var colors = depthHighlighters();
        assertEquals(8, colors.size());
        int outer = text.indexOf("((1");
        int inner = outer + 1;
        int square = text.indexOf("[2]");
        assertColor(colors, outer, 0);
        assertColor(colors, inner, 1);
        assertColor(colors, square, 2);
        int commentStart = text.indexOf("#"), commentEnd = text.indexOf('\n', commentStart);
        int stringStart = text.indexOf('"'), stringEnd = text.lastIndexOf('"') + 1;
        assertFalse(colors.stream().anyMatch(color -> color.getStartOffset() >= commentStart
                && color.getStartOffset() < commentEnd));
        assertFalse(colors.stream().anyMatch(color -> color.getStartOffset() >= stringStart
                && color.getStartOffset() < stringEnd));

        setEnabled(false);
        myFixture.doHighlighting();
        assertTrue(depthHighlighters().isEmpty());
    }

    public void testIncrementalBalancedEditRecomputesNesting() {
        String text = "def main():\n    print((1))\n";
        myFixture.configureByText(NagiFileType.HIGH, text);
        setEnabled(true);
        myFixture.doHighlighting();
        assertEquals(6, depthHighlighters().size());

        var document = myFixture.getEditor().getDocument();
        int value = document.getText().indexOf("1");
        WriteCommandAction.runWriteCommandAction(getProject(), () -> {
            document.insertString(value, "[");
            document.insertString(value + 2, "]");
        });
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        myFixture.doHighlighting();
        var colors = depthHighlighters();
        assertEquals(8, colors.size());
        assertColor(colors, value, 2);
        assertColor(colors, value + 2, 2);
    }

    public void testUnmatchedEditFallsBackToSyntaxHighlighting() {
        myFixture.configureByText(NagiFileType.LOW, "fn main() { let value = ([1); }\n");
        setEnabled(true);
        myFixture.doHighlighting();
        assertEquals("only the independent function pair may be colored", 2, depthHighlighters().size());
    }

    public void testExistingSettingsUpdatesPreserveBracketPreference() {
        setEnabled(true);
        var state = NagiSettings.getInstance().getState();
        NagiSettings.update(state.compilerPath, state.checkTimeoutSeconds);
        assertTrue(NagiSettings.getInstance().getState().bracketDepthColors);
    }

    private void setEnabled(boolean enabled) {
        var state = NagiSettings.getInstance().getState();
        NagiSettings.update(state.compilerPath, state.checkTimeoutSeconds, enabled);
        if (myFixture != null && getProject() != null && !getProject().isDisposed()) {
            DaemonCodeAnalyzer.getInstance(getProject()).restart();
        }
    }
    private List<RangeHighlighter> depthHighlighters() {
        Set<com.intellij.openapi.editor.colors.TextAttributesKey> keys = Set.of(NagiBracketColors.DEPTHS);
        return Arrays.stream(DocumentMarkupModel.forDocument(
                        myFixture.getEditor().getDocument(), getProject(), true).getAllHighlighters())
                .filter(highlighter -> highlighter.getTextAttributesKey() != null
                        && keys.contains(highlighter.getTextAttributesKey()))
                .toList();
    }
    private static void assertColor(List<RangeHighlighter> colors, int offset, int depth) {
        assertTrue("missing depth " + depth + " at " + offset, colors.stream().anyMatch(color ->
                color.getStartOffset() == offset && color.getEndOffset() == offset + 1
                        && color.getTextAttributesKey() == NagiBracketColors.DEPTHS[depth]));
    }
}
