package com.disnana.nagi;

import com.intellij.openapi.actionSystem.IdeActions;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.util.TextRange;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.Arrays;
import java.util.Locale;

/** Editor-action probe: timing is reported, while correctness uses deterministic assertions. */
public class NagiInputResponsivenessTest extends BasePlatformTestCase {
    private static final int WARMUPS = 3;
    private static final int SAMPLES = 20;
    private static final String ENTER_TAIL = "    if ready:";
    private static final String BRACKET_TAIL = "    value = call";

    public void testSmallAndTenThousandLineEditorActions() {
        measureFile(20);
        measureFile(10_000);
    }

    public void testSmallAndTenThousandLineBranchAndClosingCorrections() {
        for (int lines : new int[]{20, 10_000}) {
            measureTypedCorrection(lines, true);
            measureTypedCorrection(lines, false);
        }
    }

    private void measureTypedCorrection(int lines, boolean branch) {
        String prefix = branch ? "def main():\n    if ready:\n" : "def main():\n    call(\n";
        String original = branch ? "        else" : "        ";
        String body = branch ? "        value = 0\n" : "        value,\n";
        myFixture.configureByText("typed-" + branch + "-" + lines + ".nagi",
                prefix + body.repeat(lines - 3) + original + "<caret>");
        Editor editor = myFixture.getEditor();
        Document document = editor.getDocument();
        int tailStart = document.getLineStartOffset(lines - 1);
        String expected = (lines == 20 ? "    " : "        ") + (branch ? "else:" : ")");
        long[] samples = new long[SAMPLES];
        for (int index = -WARMUPS; index < SAMPLES; index++) {
            long started = System.nanoTime();
            myFixture.type(branch ? ':' : ')');
            long elapsed = System.nanoTime() - started;
            if (index >= 0) samples[index] = elapsed;
            assertEquals(expected, document.getText(new TextRange(tailStart, document.getTextLength())));
            replaceTail(editor, document, tailStart, original);
        }
        reportSamples(lines, branch ? "else-colon" : "closing-parenthesis", samples);
    }

    private void measureFile(int lines) {
        var source = new StringBuilder("def main():\n");
        for (int index = 0; index < lines - 2; index++) source.append("    value = 0\n");
        source.append(ENTER_TAIL).append("<caret>");
        myFixture.configureByText("typing-" + lines + ".nagi", source.toString());
        Editor editor = myFixture.getEditor();
        Document document = editor.getDocument();
        assertEquals(lines, document.getLineCount());
        int tailStart = document.getLineStartOffset(lines - 1);
        String inferred = NagiEditorIndent.afterLine(document, editor.getHighlighter(), lines - 1,
                false, 4, 4, 4);
        if (lines == 20) assertEquals("        ", inferred);
        else assertNull("A bounded scan cannot prove context across 10,000 lines", inferred);

        measureAction(lines, "Enter", editor, document, tailStart, ENTER_TAIL,
                () -> myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER));
        replaceTail(editor, document, tailStart, BRACKET_TAIL);
        measureAction(lines, "open-parenthesis", editor, document, tailStart, BRACKET_TAIL,
                () -> myFixture.type('('));
    }

    private void measureAction(int lines, String action, Editor editor, Document document,
                               int tailStart, String originalTail, Runnable type) {
        long[] samples = new long[SAMPLES];
        for (int index = -WARMUPS; index < SAMPLES; index++) {
            long started = System.nanoTime();
            type.run();
            long elapsed = System.nanoTime() - started;
            if (index >= 0) samples[index] = elapsed;
            String editedTail = document.getText(new TextRange(tailStart, document.getTextLength()));
            assertTrue("The editor action must change the final line", editedTail.length() > originalTail.length());
            assertFalse("Nagi editor actions must not insert tabs", editedTail.contains("\t"));
            replaceTail(editor, document, tailStart, originalTail);
        }
        reportSamples(lines, action, samples);
    }

    private static void reportSamples(int lines, String action, long[] samples) {
        Arrays.sort(samples);
        double medianMs = (samples[SAMPLES / 2 - 1] + samples[SAMPLES / 2]) / 2_000_000.0;
        double p95Ms = samples[(int) Math.ceil(SAMPLES * 0.95) - 1] / 1_000_000.0;
        double maxMs = samples[SAMPLES - 1] / 1_000_000.0;
        System.out.printf(Locale.ROOT,
                "NAGI_INPUT_ACTION action=%s lines=%d warmups=%d samples=%d median_ms=%.3f p95_ms=%.3f max_ms=%.3f%n",
                action, lines, WARMUPS, SAMPLES, medianMs, p95Ms, maxMs);
    }

    private void replaceTail(Editor editor, Document document, int tailStart, String replacement) {
        WriteCommandAction.runWriteCommandAction(getProject(),
                () -> document.replaceString(tailStart, document.getTextLength(), replacement));
        editor.getCaretModel().moveToOffset(document.getTextLength());
    }
}
