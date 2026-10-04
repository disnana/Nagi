package com.disnana.nagi;

import com.intellij.openapi.actionSystem.IdeActions;
import com.intellij.openapi.fileTypes.FileTypeManager;
import com.intellij.psi.PsiManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.fileEditor.FileDocumentSynchronizationVetoer;
import com.intellij.testFramework.ExtensionTestUtil;
import java.util.List;

/** Actual IntelliJ Platform editor fixtures, not API stubs. */
public class NagiEditorTest extends BasePlatformTestCase {
    public void testFileTypesAndPsi() {
        assertSame(NagiFileType.HIGH, FileTypeManager.getInstance().getFileTypeByExtension("nagi"));
        assertSame(NagiFileType.LOW, FileTypeManager.getInstance().getFileTypeByExtension("low"));
        var high = myFixture.configureByText("main.nagi", "def main():\n    print(1)\n");
        assertSame(NagiLanguage.HIGH, high.getLanguage());
        var low = myFixture.configureByText("main.low", "fn main() { print(1); }\n");
        assertSame(NagiLanguage.LOW, PsiManager.getInstance(getProject()).findFile(low.getVirtualFile()).getLanguage());
    }
    public void testHighEnterIndent() {
        myFixture.configureByText("main.nagi", "def main():<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    <caret>");
    }
    public void testLowEnterIndent() {
        myFixture.configureByText("main.low", "fn main() {<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("fn main() {\n    <caret>\n}");
    }
    public void testCommentAction() {
        myFixture.configureByText("main.nagi", "<caret>print(1)\n");
        myFixture.performEditorAction(IdeActions.ACTION_COMMENT_LINE);
        assertTrue(myFixture.getEditor().getDocument().getText().startsWith("#"));
    }
    public void testBraceCompletion() {
        myFixture.configureByText("main.low", "fn main <caret>");
        myFixture.type('(');
        myFixture.checkResult("fn main (<caret>)");
    }
    public void testLowCurlyBraceCompletion() {
        myFixture.configureByText("main.low", "fn main() <caret>");
        myFixture.type('{');
        myFixture.checkResult("fn main() {<caret>}");
    }
    public void testCompletedMultilineCallReturnsToStatementIndent() {
        myFixture.configureByText("main.nagi", "def main():\n    print(\n        42)<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    print(\n        42)\n    <caret>");
    }
    public void testContinuationDoesNotAccumulateIndent() {
        myFixture.configureByText("main.nagi", "def main():\n    values = [\n        1,<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    values = [\n        1,\n        <caret>");
    }
    public void testUnsavedManifestVetoStopsCompilerPreparation() throws Exception {
        var manifest = myFixture.getTempDirFixture().createFile("nagi.toml", "entry = \"main.nagi\"\n");
        var documents = FileDocumentManager.getInstance();
        var document = documents.getDocument(manifest);
        assertNotNull(document);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.insertString(document.getTextLength(), "# unsaved project change\n"));
        ExtensionTestUtil.maskExtensions(FileDocumentSynchronizationVetoer.EP_NAME, List.of(new FileDocumentSynchronizationVetoer() {
            @Override public boolean maySaveDocument(com.intellij.openapi.editor.Document candidate, boolean explicit) { return candidate != document; }
        }), getTestRootDisposable());
        try {
            assertFalse(NagiCompilerAction.saveInputs());
            assertTrue(documents.isDocumentUnsaved(document));
        } finally { documents.reloadFromDisk(document); }
    }
}
