package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.openapi.actionSystem.IdeActions;
import com.intellij.psi.codeStyle.CodeStyleSettingsManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;

/** Contextual editing through the real IDE actions, without a Python dependency. */
public class NagiPythonEditingTest extends BasePlatformTestCase {
    private void enter() { myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER); }

    public void testRepeatedEnterExitsOneNestedBlockAtATime() {
        myFixture.configureByText("main.nagi", "def main():\n    if ready:\n        print(1)<caret>");
        enter();
        myFixture.checkResult("def main():\n    if ready:\n        print(1)\n        <caret>");
        enter();
        myFixture.checkResult("def main():\n    if ready:\n        print(1)\n        \n    <caret>");
        enter();
        myFixture.checkResult("def main():\n    if ready:\n        print(1)\n        \n    \n<caret>");
        enter();
        assertEquals(0, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
    }

    public void testManualBackspaceDedentIsNotUndoneOrRepeated() {
        myFixture.configureByText("main.nagi", "def main():\n    if ready:\n        print(1)<caret>");
        enter();
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
        enter();
        myFixture.checkResult("def main():\n    if ready:\n        print(1)\n    \n    <caret>");
    }

    public void testExplicitEmptyIndentIsPreserved() {
        myFixture.configureByText("main.nagi", "def main():\n    if ready:\n        print(1)\n  <caret>");
        enter();
        myFixture.checkResult("def main():\n    if ready:\n        print(1)\n  \n  <caret>");
    }

    public void testTypingInvalidatesConsecutiveEnter() {
        myFixture.configureByText("main.nagi", "def main():<caret>");
        enter();
        myFixture.type("  ");
        enter();
        myFixture.checkResult("def main():\n      \n      <caret>");
    }

    public void testUndoRedoDoesNotReuseAnEarlierEnterGesture() {
        myFixture.configureByText("main.nagi", "def main():<caret>");
        enter();
        myFixture.performEditorAction(IdeActions.ACTION_UNDO);
        myFixture.performEditorAction(IdeActions.ACTION_REDO);
        enter();
        myFixture.checkResult("def main():\n    \n    <caret>");
    }

    public void testBlankContinuationDoesNotExitCallOrList() {
        for (String opening : new String[]{"call(", "["}) {
            myFixture.configureByText("main.nagi", "def main():\n    value = " + opening + "<caret>");
            enter();
            enter();
            assertEquals(8, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
        }
    }

    public void testLowBlankLineKeepsBraceContext() {
        myFixture.configureByText("main.low", "fn main() {\n    print(1);<caret>\n}");
        enter();
        enter();
        assertEquals(4, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
        assertTrue(myFixture.getEditor().getDocument().getText().endsWith("\n}"));
    }

    public void testCommentAndQuotedColonDoNotExitOnFirstEnter() {
        for (String statement : new String[]{"# return:", "print(\"return: [\")"}) {
            myFixture.configureByText("main.nagi", "def main():\n    " + statement + "<caret>");
            enter();
            assertEquals(4, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
        }
    }

    public void testUnknownDelimiterContextDoesNotGuessDedent() {
        myFixture.configureByText("main.nagi", "call(\n" + "    item,\n".repeat(1100) + "    item<caret>");
        enter();
        enter();
        assertEquals(4, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
    }

    public void testSmartIndentOffDisablesRepeatedEnterDedent() {
        var settings = CodeInsightSettings.getInstance();
        boolean previous = settings.SMART_INDENT_ON_ENTER;
        try {
            settings.SMART_INDENT_ON_ENTER = false;
            myFixture.configureByText("main.nagi", "def main():\n    print(1)<caret>");
            enter();
            enter();
            assertEquals(4, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
        } finally {
            settings.SMART_INDENT_ON_ENTER = previous;
        }
    }

    public void testTwoSpaceStyleUsesOneConfiguredStep() {
        var settings = CodeStyle.getSettings(getProject()).clone();
        settings.getLanguageIndentOptions(NagiLanguage.HIGH).INDENT_SIZE = 2;
        var previous = CodeStyleSettingsManager.getInstance(getProject()).getTemporarySettings();
        CodeStyle.setTemporarySettings(getProject(), settings);
        try {
            myFixture.configureByText("main.nagi", "def main():\n  if ready:\n    print(1)<caret>");
            enter();
            enter();
            assertEquals(2, myFixture.getEditor().getCaretModel().getLogicalPosition().column);
        } finally {
            if (previous == null) CodeStyle.dropTemporarySettings(getProject());
            else CodeStyle.setTemporarySettings(getProject(), previous);
        }
    }
}
