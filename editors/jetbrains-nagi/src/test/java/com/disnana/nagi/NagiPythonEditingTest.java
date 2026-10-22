package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.editorActions.enter.EnterHandlerDelegate;
import com.intellij.codeInsight.editorActions.enter.EnterHandlerDelegateAdapter;
import com.intellij.openapi.actionSystem.DataContext;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.editor.ex.DocumentEx;
import com.intellij.openapi.editor.actionSystem.EditorActionHandler;
import com.intellij.openapi.util.Ref;
import com.intellij.psi.PsiFile;
import com.intellij.testFramework.ExtensionTestUtil;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
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

    public void testDefaultZeroSequenceDoesNotEnableRepeatedEnter() {
        myFixture.configureByText("main.nagi", "def main():<caret>");
        maskSequence(false);
        enter();
        enter();
        myFixture.checkResult("def main():\n    \n    <caret>");
    }

    public void testIncreasingSequenceStartingAtZeroSupportsTwoEnters() {
        myFixture.configureByText("main.nagi", "def main():<caret>");
        maskSequence(true);
        enter();
        enter();
        myFixture.checkResult("def main():\n    \n<caret>");
    }

    /** Keep real platform actions/document edits; mask only our delegate's sequence capability. */
    private void maskSequence(boolean increasing) {
        Editor actualEditor = myFixture.getEditor();
        DocumentEx actualDocument = (DocumentEx) actualEditor.getDocument();
        int baseline = actualDocument.getModificationSequence();
        DocumentEx document = (DocumentEx) Proxy.newProxyInstance(DocumentEx.class.getClassLoader(),
                new Class<?>[]{DocumentEx.class}, (proxy, method, args) -> {
                    if (method.getName().equals("getModificationSequence"))
                        return increasing ? actualDocument.getModificationSequence() - baseline : 0;
                    try { return method.invoke(actualDocument, args); }
                    catch (InvocationTargetException e) { throw e.getCause(); }
                });
        Editor editor = (Editor) Proxy.newProxyInstance(Editor.class.getClassLoader(),
                new Class<?>[]{Editor.class}, (proxy, method, args) -> {
                    if (method.getName().equals("getDocument")) return document;
                    try { return method.invoke(actualEditor, args); }
                    catch (InvocationTargetException e) { throw e.getCause(); }
                });
        var delegates = new ArrayList<EnterHandlerDelegate>();
        int replaced = 0;
        for (var delegate : EnterHandlerDelegate.EP_NAME.getExtensionList()) {
            if (!(delegate instanceof NagiEnterHandler)) { delegates.add(delegate); continue; }
            replaced++;
            delegates.add(new EnterHandlerDelegateAdapter() {
                @Override public Result preprocessEnter(PsiFile file, Editor ignored, Ref<Integer> offset,
                        Ref<Integer> advance, DataContext context, EditorActionHandler original) {
                    return delegate.preprocessEnter(file, editor, offset, advance, context, original);
                }
                @Override public Result postProcessEnter(PsiFile file, Editor ignored, DataContext context) {
                    return delegate.postProcessEnter(file, editor, context);
                }
            });
        }
        assertEquals("Exactly one Nagi Enter delegate must be exercised", 1, replaced);
        ExtensionTestUtil.maskExtensions(EnterHandlerDelegate.EP_NAME, delegates, getTestRootDisposable());
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
