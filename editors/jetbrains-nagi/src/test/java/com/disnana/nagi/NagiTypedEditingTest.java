package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.psi.codeStyle.CodeStyleSettingsManager;
import com.intellij.psi.codeStyle.CommonCodeStyleSettings;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.function.Consumer;

/** Actual typed editor input exercises the plugin registration and document edits. */
public class NagiTypedEditingTest extends BasePlatformTestCase {
    public void testBareElifAnnotationKeepsItsNestedIndentation() {
        String prefix = "def main():\n    if ready:\n        print(1)\n        ";
        for (String name : new String[] {"elif", "elif "}) {
            myFixture.configureByText("annotation.nagi", prefix + name + "<caret>");
            myFixture.type(':');
            myFixture.checkResult(prefix + name + ":<caret>");
            myFixture.type(" i64 = 2");
            myFixture.checkResult(prefix + name + ": i64 = 2<caret>");
        }
    }
    public void testElifChainAndFinalElseAlignToTheSameIf() {
        myFixture.configureByText("chain.nagi", "if a:\n    work()\nelif b:\n    work()\n    elif c<caret>");
        myFixture.type(':');
        myFixture.checkResult("if a:\n    work()\nelif b:\n    work()\nelif c:<caret>");
        myFixture.configureByText("final.nagi", "if a:\n    work()\nelif b:\n    work()\n    else<caret>");
        myFixture.type(':');
        myFixture.checkResult("if a:\n    work()\nelif b:\n    work()\nelse:<caret>");
    }
    public void testElseAlignsToNearestNestedIfAndOuterIfSurvivesInnerElse() {
        myFixture.configureByText("main.nagi", "def main():\n    if a:\n        if b:\n            print(1)\n        else:\n            print(2)\n            else<caret>");
        myFixture.type(':');
        myFixture.checkResult("def main():\n    if a:\n        if b:\n            print(1)\n        else:\n            print(2)\n    else:<caret>");
    }

    public void testConsumedIfDoesNotOwnASecondElse() {
        myFixture.configureByText("main.nagi", "if ready:\n    work()\nelse:\n    other()\n    else<caret>");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    work()\nelse:\n    other()\n    else:<caret>");
    }

    public void testCaseAlignsUnderNearestNestedMatch() {
        myFixture.configureByText("main.nagi", "match first:\n    case One:\n        match second:\n            case Two:\n                work()\n                case Three<caret>");
        myFixture.type(':');
        myFixture.checkResult("match first:\n    case One:\n        match second:\n            case Two:\n                work()\n            case Three:<caret>");
    }

    public void testMultilineCaseHeaderMovesWithItsContinuationLines() {
        myFixture.configureByText("main.nagi", "match result:\n    case Ok(value):\n        print(value)\n        case Err(\n            problem\n        )<caret>");
        myFixture.type(':');
        myFixture.checkResult("match result:\n    case Ok(value):\n        print(value)\n    case Err(\n        problem\n    ):<caret>");
    }

    public void testNoBranchOwnerLeavesTypedColonAndWhitespaceAlone() {
        myFixture.configureByText("main.nagi", "work()\n        else<caret>");
        myFixture.type(':');
        myFixture.checkResult("work()\n        else:<caret>");
    }

    public void testColonInsideStringAndCommentDoesNotAlign() {
        myFixture.configureByText("string.nagi", "if ready:\n    work()\n        \"else<caret>\"");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    work()\n        \"else:<caret>\"");

        myFixture.configureByText("comment.nagi", "if ready:\n    work()\n        # else<caret>");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    work()\n        # else:<caret>");
    }

    public void testUnicodeBeforeBranchDoesNotCorruptOffsets() {
        myFixture.configureByText("unicode.nagi", "if ready:\n    print(\"猫 🔒\")\n        else<caret>");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    print(\"猫 🔒\")\nelse:<caret>");
    }

    public void testTypedHighClosingDelimiterUsesOpenerLineIndentation() {
        myFixture.configureByText("main.nagi", "def main():\n    call(\n        <caret>");
        myFixture.type(')');
        myFixture.checkResult("def main():\n    call(\n    )<caret>");

        myFixture.configureByText("bracket.nagi", "values = [\n            <caret>");
        myFixture.type(']');
        myFixture.checkResult("values = [\n]<caret>");
    }

    public void testNonLeadingTypedAndOvertypedClosersKeepTheirCodeLineIndent() {
        myFixture.configureByText("typed.nagi", "    call(\n        value<caret>");
        myFixture.type(')');
        myFixture.checkResult("    call(\n        value)<caret>");

        myFixture.configureByText("overtype.nagi", "    call(\n        value)<caret>");
        int closer = myFixture.getEditor().getDocument().getText().lastIndexOf(')');
        myFixture.getEditor().getCaretModel().moveToOffset(closer);
        myFixture.type(')');
        myFixture.checkResult("    call(\n        value)<caret>");
    }

    public void testLongSuffixIsPreservedWithoutBecomingCorrectionContext() {
        String suffix = " # " + "x".repeat(20_000);
        myFixture.configureByText("suffix.nagi", "    call(\n        <caret>" + suffix);
        myFixture.type(')');
        myFixture.checkResult("    call(\n    )<caret>" + suffix);
    }

    public void testSelectionReplacementDoesNotAlignACloser() {
        myFixture.configureByText("selection.nagi", "call(\n        <selection>x</selection>");
        myFixture.type(')');
        myFixture.checkResult("call(\n        )<caret>");
    }

    public void testOvertypedHighCloserAlignsBeforePlatformSkipsIt() {
        myFixture.configureByText("main.nagi", "    call(\n        value\n        )<caret>");
        // The platform recognizes the existing pair and moves over ')'. The typed
        // handler must still align its line during beforeCharTyped.
        myFixture.getEditor().getCaretModel().moveToOffset(
                myFixture.getEditor().getDocument().getText().indexOf(')'));
        myFixture.type(')');
        myFixture.checkResult("    call(\n        value\n    )<caret>");
    }

    public void testLowClosingBraceAlignsAndHighCurlyBraceIsIgnored() {
        myFixture.configureByText("main.low", "fn main() {\n    if true {\n        work();\n            <caret>");
        myFixture.type('}');
        myFixture.checkResult("fn main() {\n    if true {\n        work();\n    }<caret>");

        myFixture.configureByText("main.nagi", "value<caret>");
        myFixture.type('}');
        myFixture.checkResult("value}<caret>");
    }

    public void testStringsAndCommentsDoNotSupplyClosingDelimiters() {
        myFixture.configureByText("string.nagi", "    print(\"value <caret>\")");
        myFixture.type(')');
        myFixture.checkResult("    print(\"value )<caret>\")");

        myFixture.configureByText("comment.nagi", "    # value <caret>");
        myFixture.type(')');
        myFixture.checkResult("    # value )<caret>");
    }

    public void testMismatchedDelimiterContextDoesNotGuess() {
        myFixture.configureByText("mismatch.nagi", "    call([\n        <caret>");
        myFixture.type(')');
        myFixture.checkResult("    call([\n        )<caret>");
    }

    public void testLongPrefixAndUnknownWholeBranchContextFallBack() {
        String longPrefix = "if ready:\n" + "    work()\n".repeat(100) + "        else<caret>";
        myFixture.configureByText("long.nagi", longPrefix);
        myFixture.type(':');
        myFixture.checkResult("if ready:\n" + "    work()\n".repeat(100) + "        else:<caret>");

        myFixture.configureByText("unknown.nagi", "if ready:\n    work(]\n        else<caret>");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    work(]\n        else:<caret>");
    }

    public void testEffectiveIndentAndTabWidthsProduceSpacesOnly() {
        withIndentOptions(NagiLanguage.HIGH, options -> {
            options.INDENT_SIZE = 2;
            options.TAB_SIZE = 8;
            options.USE_TAB_CHARACTER = true;
        }, () -> {
            myFixture.configureByText("branch.nagi", "match value:\n    case First:\n        work()\n            case Second<caret>");
            myFixture.type(':');
            myFixture.checkResult("match value:\n    case First:\n        work()\n  case Second:<caret>");

            myFixture.configureByText("closer.nagi", "\tcall(\n            <caret>");
            myFixture.type(')');
            myFixture.checkResult("\tcall(\n        )<caret>");
            assertFalse(myFixture.getEditor().getDocument().getText().contains("\n\t"));
        });
    }

    public void testOversizedCodeStyleWidthsFailClosed() {
        var file = myFixture.configureByText("branch.nagi", "match value:\n        case Other:");
        String before = myFixture.getEditor().getDocument().getText();
        var indentOptions = (CommonCodeStyleSettings.IndentOptions) CodeStyle.getIndentOptions(file).clone();
        indentOptions.INDENT_SIZE = Integer.MAX_VALUE;
        NagiTypedIndent.alignTypedColon(file, myFixture.getEditor(), before.lastIndexOf(':'), indentOptions);
        assertEquals(before, myFixture.getEditor().getDocument().getText());

        file = myFixture.configureByText("closer.nagi", "\tcall(\n        )");
        before = myFixture.getEditor().getDocument().getText();
        indentOptions = (CommonCodeStyleSettings.IndentOptions) CodeStyle.getIndentOptions(file).clone();
        indentOptions.TAB_SIZE = Integer.MAX_VALUE;
        NagiTypedIndent.alignTypedCloser(file, myFixture.getEditor(), ')', before.lastIndexOf(')'), indentOptions);
        assertEquals(before, myFixture.getEditor().getDocument().getText());
    }

    public void testSmartIndentOffAndSelectionGuardLeaveBranchWhitespaceAlone() {
        CodeInsightSettings settings = CodeInsightSettings.getInstance();
        boolean previous = settings.SMART_INDENT_ON_ENTER;
        try {
            settings.SMART_INDENT_ON_ENTER = false;
            myFixture.configureByText("off.nagi", "if ready:\n    work()\n        else<caret>");
            myFixture.type(':');
            myFixture.checkResult("if ready:\n    work()\n        else:<caret>");
        } finally {
            settings.SMART_INDENT_ON_ENTER = previous;
        }

        myFixture.configureByText("selection.nagi", "if ready:\n    else<selection>x</selection>");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    else:<caret>");
    }

    public void testMulticaretTypingDoesNotApplyBranchAlignment() {
        myFixture.configureByText("multicaret.nagi", "if ready:\n        else<caret>");
        var editor = myFixture.getEditor();
        assertNotNull(editor.getCaretModel().addCaret(editor.offsetToVisualPosition(0)));
        myFixture.type(':');
        assertEquals(2, editor.getCaretModel().getCaretCount());
        assertTrue(editor.getDocument().getText().contains("        else:"));
    }

    public void testTypedCorrectionAndColonUndoAsOneDeterministicInput() {
        myFixture.configureByText("undo.nagi", "if ready:\n    work()\n        else<caret>");
        myFixture.type(':');
        myFixture.checkResult("if ready:\n    work()\nelse:<caret>");
        myFixture.performEditorAction(com.intellij.openapi.actionSystem.IdeActions.ACTION_UNDO);
        myFixture.checkResult("if ready:\n    work()\n        else<caret>");
    }

    private void withIndentOptions(NagiLanguage language,
                                   Consumer<CommonCodeStyleSettings.IndentOptions> configure,
                                   Runnable test) {
        var settings = CodeStyle.getSettings(getProject()).clone();
        configure.accept(settings.getLanguageIndentOptions(language));
        var previous = CodeStyleSettingsManager.getInstance(getProject()).getTemporarySettings();
        CodeStyle.setTemporarySettings(getProject(), settings);
        try {
            test.run();
        } finally {
            if (previous == null) CodeStyle.dropTemporarySettings(getProject());
            else CodeStyle.setTemporarySettings(getProject(), previous);
        }
    }
}
