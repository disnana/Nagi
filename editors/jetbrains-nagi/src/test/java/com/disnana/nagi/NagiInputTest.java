package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.openapi.actionSystem.IdeActions;
import com.intellij.openapi.ide.CopyPasteManager;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.Caret;
import com.intellij.openapi.fileEditor.FileEditorManager;
import com.intellij.openapi.fileTypes.PlainTextFileType;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.util.ThrowableComputable;
import com.intellij.openapi.util.registry.Registry;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.openapi.vfs.VirtualFile;
import com.intellij.psi.codeStyle.CodeStyleSettings;
import com.intellij.psi.codeStyle.CommonCodeStyleSettings;
import com.intellij.psi.codeStyle.FileIndentOptionsProvider;
import com.intellij.psi.codeStyle.CodeStyleSettingsManager;
import com.intellij.psi.PsiManager;
import com.intellij.testFramework.ExtensionTestUtil;
import com.intellij.testFramework.PlatformTestUtil;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.awt.datatransfer.StringSelection;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/** Real editor actions exercise the Nagi file-type registrations and caret behavior. */
public class NagiInputTest extends BasePlatformTestCase {
    public void testHighQuotesPairSkipAndDelete() {
        myFixture.configureByText("main.nagi", "print(<caret>)");
        myFixture.type('"');
        myFixture.checkResult("print(\"<caret>\")");
        myFixture.type("hello\"");
        myFixture.checkResult("print(\"hello\"<caret>)");

        myFixture.configureByText("other.nagi", "value = <caret>");
        myFixture.type('\'');
        myFixture.checkResult("value = '<caret>'");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
        myFixture.checkResult("value = <caret>");
    }

    public void testLowQuotesAndCommentContext() {
        myFixture.configureByText("main.low", "let text = <caret>;");
        myFixture.type('"');
        myFixture.checkResult("let text = \"<caret>\";");
        myFixture.configureByText("comment.nagi", "# <caret>");
        myFixture.type('"');
        myFixture.checkResult("# \"<caret>");
    }

    public void testQuoteWrapsSelection() {
        var smartKeys = CodeInsightSettings.getInstance();
        boolean previous = smartKeys.SURROUND_SELECTION_ON_QUOTE_TYPED;
        try {
            smartKeys.SURROUND_SELECTION_ON_QUOTE_TYPED = true;
            myFixture.configureByText("main.nagi", "value = <selection>name</selection>");
            myFixture.type('"');
            myFixture.checkResult("value = \"<selection>name<caret></selection>\"");
        } finally {
            smartKeys.SURROUND_SELECTION_ON_QUOTE_TYPED = previous;
        }
    }

    public void testLowBracketsPairSkipAndDelete() {
        myFixture.configureByText("main.low", "fn main() { value = <caret>; }");
        myFixture.type('[');
        myFixture.checkResult("fn main() { value = [<caret>]; }");
        myFixture.type(']');
        myFixture.checkResult("fn main() { value = []<caret>; }");
        myFixture.configureByText("other.low", "fn main() <caret>");
        myFixture.type('{');
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_BACKSPACE);
        myFixture.checkResult("fn main() <caret>");
    }

    public void testBracketWrapsSelection() {
        myFixture.configureByText("main.nagi", "print<selection>value</selection>");
        myFixture.type('(');
        assertEquals("print(value)", myFixture.getEditor().getDocument().getText());
        assertEquals("value", myFixture.getEditor().getSelectionModel().getSelectedText());
    }

    public void testBracketPairingRespectsSmartKeysOff() {
        var smartKeys = CodeInsightSettings.getInstance();
        boolean previous = smartKeys.AUTOINSERT_PAIR_BRACKET;
        try {
            smartKeys.AUTOINSERT_PAIR_BRACKET = false;
            myFixture.configureByText("main.nagi", "print<caret>");
            myFixture.type('(');
            myFixture.checkResult("print(<caret>");
        } finally {
            smartKeys.AUTOINSERT_PAIR_BRACKET = previous;
        }
    }

    public void testQuotePairingRespectsSmartKeysOff() {
        var smartKeys = CodeInsightSettings.getInstance();
        boolean previous = smartKeys.AUTOINSERT_PAIR_QUOTE;
        try {
            smartKeys.AUTOINSERT_PAIR_QUOTE = false;
            myFixture.configureByText("main.nagi", "value = <caret>");
            myFixture.type('"');
            myFixture.checkResult("value = \"<caret>");
        } finally {
            smartKeys.AUTOINSERT_PAIR_QUOTE = previous;
        }
    }

    public void testHighReturnDedentsOneLevel() {
        myFixture.configureByText("main.nagi", "def main():\n    if ready:\n        return 1<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    if ready:\n        return 1\n    <caret>");
    }

    public void testEnterAfterEmptyLineHonorsExplicitHighDedent() {
        myFixture.configureByText("main.nagi", "def main():\n\n<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n\n\n<caret>");
    }

    public void testMultilineReturnDedentsAfterClosingCall() {
        myFixture.configureByText("main.nagi", "def main():\n    if ready:\n        return call(\n            1)<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    if ready:\n        return call(\n            1)\n    <caret>");
    }

    public void testIndentWidthsFollowCodeStyle() {
        withHighIndentOptions(options -> {
            options.INDENT_SIZE = 2;
            options.CONTINUATION_INDENT_SIZE = 8;
        }, () -> {
            myFixture.configureByText("main.nagi", "def main():<caret>");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
            myFixture.checkResult("def main():\n  <caret>");
            myFixture.configureByText("next.nagi", "def main():\n  values = [<caret>");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
            myFixture.checkResult("def main():\n  values = [\n          <caret>");
        });
    }

    public void testOtherConfiguredSpaceWidthIsPreserved() {
        withHighIndentOptions(options -> options.INDENT_SIZE = 3, () -> {
            var file = myFixture.configureByText("main.nagi", "def main():<caret>");
            assertEquals(3, CodeStyle.getIndentOptions(file).INDENT_SIZE);
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
            myFixture.checkResult("def main():\n   <caret>");
        });
    }

    public void testSmartIndentOffLeavesPlatformIndent() {
        var smartKeys = CodeInsightSettings.getInstance();
        boolean previous = smartKeys.SMART_INDENT_ON_ENTER;
        try {
            smartKeys.SMART_INDENT_ON_ENTER = false;
            myFixture.configureByText("main.nagi", "def main():\n    return 1<caret>");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
            myFixture.checkResult("def main():\n    return 1\n    <caret>");
        } finally {
            smartKeys.SMART_INDENT_ON_ENTER = previous;
        }
    }

    public void testLongUnknownDelimiterContextLeavesPlatformIndent() {
        String source = "call(\n" + "    item,\n".repeat(1100) + "    case value:<caret>";
        myFixture.configureByText("main.nagi", source);
        var editor = myFixture.getEditor();
        var document = editor.getDocument();
        int line = document.getLineNumber(editor.getCaretModel().getOffset());
        assertNull(NagiEditorIndent.afterLine(document, editor.getHighlighter(), line, false, 4, 4, 4));
        myFixture.configureByText("return.nagi", "call(\n" + "    item,\n".repeat(1100) + "    return value<caret>");
        editor = myFixture.getEditor();
        document = editor.getDocument();
        line = document.getLineNumber(editor.getCaretModel().getOffset());
        assertNull(NagiEditorIndent.afterLine(document, editor.getHighlighter(), line, false, 4, 4, 4));
    }

    public void testUnicodeAndCommentColonDoNotOpenBlock() {
        myFixture.configureByText("main.nagi", "def main():\n    print(\"猫:🔒\") # trailing:<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    print(\"猫:🔒\") # trailing:\n    <caret>");
    }

    public void testTabAndShiftTabUseSpaces() {
        withHighIndentOptions(options -> options.INDENT_SIZE = 2, () -> {
            myFixture.configureByText("main.nagi", "def main():\n<caret>value = 1");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_TAB);
            myFixture.checkResult("def main():\n  <caret>value = 1");
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_UNINDENT_SELECTION);
            myFixture.checkResult("def main():\n<caret>value = 1");
        });
    }

    public void testTabUsesSpacesWhenTabSettingIsEnabled() {
        withHighIndentOptions(options -> options.USE_TAB_CHARACTER = true, () -> {
            var file = myFixture.configureByText("main.nagi", "def main():\n<caret>value = 1");
            assertFalse(CodeStyle.getIndentOptions(file).USE_TAB_CHARACTER);
            assertFalse(myFixture.getEditor().getSettings().isUseTabCharacter(getProject()));
            myFixture.performEditorAction(IdeActions.ACTION_EDITOR_TAB);
            assertEquals("def main():\n    value = 1", myFixture.getEditor().getDocument().getText());
        });
    }

    public void testEditorConfigTabStyleUsesSpacesAndPreservesWidths() {
        Registry.get("editor.config.full.settings.support").setValue(false, getTestRootDisposable());
        myFixture.addFileToProject(".editorconfig", "root = true\n\n[*.nagi]\nindent_style = tab\nindent_size = 6\ntab_width = 8\n\n[*.low]\nindent_style = tab\nindent_size = 6\ntab_width = 8\n\n[*.txt]\nindent_style = tab\nindent_size = 6\ntab_width = 8\n");
        var file = myFixture.configureByText("editorconfig.nagi", "def main():\n<caret>value = 1");
        var editorConfigProvider = FileIndentOptionsProvider.EP_NAME.getExtensionList().stream()
                .filter(candidate -> candidate.getClass().getName().contains("EditorConfigIndentOptionsProvider"))
                .findFirst().orElseThrow();
        var requested = editorConfigProvider.getIndentOptions(getProject(), CodeStyle.getSettings(file), file.getVirtualFile());
        assertNotNull("The bundled EditorConfig provider must match the project file; full settings="
                + Registry.is("editor.config.full.settings.support"), requested);
        assertEquals(6, requested.INDENT_SIZE);
        assertEquals(8, requested.TAB_SIZE);
        assertTrue(requested.USE_TAB_CHARACTER);

        var effective = CodeStyle.getIndentOptions(file);
        assertEquals(6, effective.INDENT_SIZE);
        assertEquals(8, effective.TAB_SIZE);
        assertTrue("EditorConfig is the effective code-style provider here", effective.USE_TAB_CHARACTER);
        assertFalse("Nagi editor actions must insert spaces", myFixture.getEditor().getSettings().isUseTabCharacter(getProject()));
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_TAB);
        myFixture.checkResult("def main():\n      <caret>value = 1");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_UNINDENT_SELECTION);
        myFixture.checkResult("def main():\n<caret>value = 1");

        myFixture.configureByText("selection.nagi", "<selection>value = 1\nother = 2</selection>");
        assertFalse(myFixture.getEditor().getSettings().isUseTabCharacter(getProject()));
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_INDENT_SELECTION);
        assertEquals("      value = 1\n      other = 2", myFixture.getEditor().getDocument().getText());
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_UNINDENT_SELECTION);
        assertEquals("value = 1\nother = 2", myFixture.getEditor().getDocument().getText());

        myFixture.configureByText("enter.nagi", "def main():<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n      <caret>");

        myFixture.configureByText("paste.nagi", "def main():\n      <caret>");
        CopyPasteManager.getInstance().setContents(new StringSelection("value = 1\nvalue = 2"));
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_PASTE);
        myFixture.checkResult("def main():\n      value = 1\n      value = 2<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_REFORMAT);
        assertEquals("def main():\n      value = 1\n      value = 2", myFixture.getEditor().getDocument().getText());
        assertFalse(myFixture.getEditor().getDocument().getText().contains("\t"));
        assertEquals(6, CodeStyle.getIndentOptions(myFixture.getFile()).INDENT_SIZE);

        var low = myFixture.configureByText("editorconfig.low", "fn main() {\n<caret>value = 1;\n}");
        assertEquals(6, CodeStyle.getIndentOptions(low).INDENT_SIZE);
        assertTrue(CodeStyle.getIndentOptions(low).USE_TAB_CHARACTER);
        assertFalse(myFixture.getEditor().getSettings().isUseTabCharacter(getProject()));
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_TAB);
        myFixture.checkResult("fn main() {\n      <caret>value = 1;\n}");

        var other = myFixture.configureByText("notes.txt", "plain text");
        assertTrue(CodeStyle.getIndentOptions(other).USE_TAB_CHARACTER);
        assertTrue("Non-Nagi editors keep their tab policy", myFixture.getEditor().getSettings().isUseTabCharacter(getProject()));
    }

    public void testOpenEditorFileTypeRenameUpdatesTabPolicy() throws Exception {
        Registry.get("editor.config.full.settings.support").setValue(false, getTestRootDisposable());
        withPhysicalFile("renamed.nagi", "def main():\nvalue = 1",
                "root = true\n\n[*.nagi]\nindent_style = tab\nindent_size = 6\ntab_width = 8\n\n[*.txt]\nindent_style = tab\nindent_size = 6\ntab_width = 8\n", virtualFile -> {
            myFixture.configureFromExistingVirtualFile(virtualFile);
            var editor = myFixture.getEditor();
            assertFalse(editor.getSettings().isUseTabCharacter(getProject()));
            rename(virtualFile, "renamed.txt");
            PlatformTestUtil.waitWithEventsDispatching("Text editor did not recover its tab policy", () ->
                    editor.getSettings().isUseTabCharacter(getProject()), 5);
            assertFalse(editor.isDisposed());
            assertEquals("txt", virtualFile.getExtension());
            var textFile = PsiManager.getInstance(getProject()).findFile(virtualFile);
            assertNotNull(textFile);
            assertTrue(CodeStyle.getIndentOptions(textFile).USE_TAB_CHARACTER);
            assertTrue("The open text editor must recover its tab policy",
                    editor.getSettings().isUseTabCharacter(getProject()));
            rename(virtualFile, "renamed.nagi");
            PlatformTestUtil.waitWithEventsDispatching("Renamed Nagi editor did not use spaces", () ->
                    !editor.getSettings().isUseTabCharacter(getProject()), 5);
            assertFalse("The renamed Nagi editor must use spaces",
                    editor.getSettings().isUseTabCharacter(getProject()));
        });
    }

    public void testOpenTextEditorRenamedToLowStartsUsingSpaces() throws Exception {
        Registry.get("editor.config.full.settings.support").setValue(false, getTestRootDisposable());
        withPhysicalFile("renamed.txt", "let value = 1;",
                "root = true\n\n[*.txt]\nindent_style = tab\nindent_size = 6\n"
                        + "\n[*.low]\nindent_style = tab\nindent_size = 6\n", virtualFile -> {
            myFixture.configureFromExistingVirtualFile(virtualFile);
            var editor = myFixture.getEditor();
            assertTrue(CodeStyle.getIndentOptions(getProject(), virtualFile).USE_TAB_CHARACTER);
            assertTrue(editor.getSettings().isUseTabCharacter(getProject()));
            rename(virtualFile, "renamed.low");
            PlatformTestUtil.waitWithEventsDispatching("Renamed Low editor did not use spaces", () ->
                    !editor.getSettings().isUseTabCharacter(getProject()), 5);
            assertEquals("low", virtualFile.getExtension());
            assertTrue(CodeStyle.getIndentOptions(getProject(), virtualFile).USE_TAB_CHARACTER);
        });
    }

    public void testRenamedTextEditorTracksProjectCodeStyleChanges() throws Exception {
        Registry.get("editor.config.full.settings.support").setValue(false, getTestRootDisposable());
        var manager = CodeStyleSettingsManager.getInstance(getProject());
        var previous = manager.getTemporarySettings();
        var tabs = CodeStyle.getSettings(getProject()).clone();
        tabs.getIndentOptions(PlainTextFileType.INSTANCE).USE_TAB_CHARACTER = true;
        CodeStyle.setTemporarySettings(getProject(), tabs);
        try {
            withPhysicalFile("style.nagi", "value = 1",
                    "root = true\n\n[*.nagi]\nindent_style = tab\nindent_size = 6\n", virtualFile -> {
                myFixture.configureFromExistingVirtualFile(virtualFile);
                var editor = myFixture.getEditor();
                assertFalse(editor.getSettings().isUseTabCharacter(getProject()));
                rename(virtualFile, "style.txt");
                PlatformTestUtil.waitWithEventsDispatching("Text editor did not adopt project tabs", () ->
                        editor.getSettings().isUseTabCharacter(getProject()), 5);
                assertTrue(CodeStyle.getIndentOptions(getProject(), virtualFile).USE_TAB_CHARACTER);

                var spaces = tabs.clone();
                spaces.getIndentOptions(PlainTextFileType.INSTANCE).USE_TAB_CHARACTER = false;
                CodeStyle.setTemporarySettings(getProject(), spaces);
                manager.notifyCodeStyleSettingsChanged();
                PlatformTestUtil.waitWithEventsDispatching("Text editor retained stale tabs", () ->
                        !editor.getSettings().isUseTabCharacter(getProject()), 5);
                assertFalse(CodeStyle.getIndentOptions(getProject(), virtualFile).USE_TAB_CHARACTER);
            });
        } finally {
            if (previous == null) CodeStyle.dropTemporarySettings(getProject());
            else CodeStyle.setTemporarySettings(getProject(), previous);
        }
    }

    public void testIndentProviderPreservesPerFileWidthsAndBoundsDelegation() {
        var calls = new AtomicInteger();
        var offered = new AtomicReference<CommonCodeStyleSettings.IndentOptions>();
        var provider = new NagiIndentOptionsProvider();
        FileIndentOptionsProvider perFile = new FileIndentOptionsProvider() {
            @Override public boolean useOnFullReformat() { return true; }
            @Override public CommonCodeStyleSettings.IndentOptions getIndentOptions(Project project,
                                                                                     CodeStyleSettings settings, VirtualFile file) {
                if (file.getFileType() != NagiFileType.HIGH) return null;
                calls.incrementAndGet();
                var options = (CommonCodeStyleSettings.IndentOptions)
                        settings.getLanguageIndentOptions(NagiLanguage.HIGH).clone();
                options.INDENT_SIZE = 6;
                options.CONTINUATION_INDENT_SIZE = 10;
                options.TAB_SIZE = 8;
                options.USE_TAB_CHARACTER = true;
                options.SMART_TABS = true;
                offered.set(options);
                return options;
            }
        };
        ExtensionTestUtil.maskExtensions(FileIndentOptionsProvider.EP_NAME, List.of(provider, perFile), getTestRootDisposable());
        var file = myFixture.configureByText("main.nagi", "def main():\n<caret>value = 1");
        calls.set(0);
        var effective = provider.getIndentOptions(getProject(), CodeStyle.getSettings(file), file.getVirtualFile());
        assertNotNull(effective);
        assertEquals(1, calls.get());
        assertEquals(6, effective.INDENT_SIZE);
        assertEquals(10, effective.CONTINUATION_INDENT_SIZE);
        assertEquals(8, effective.TAB_SIZE);
        assertFalse(effective.USE_TAB_CHARACTER);
        assertFalse(effective.SMART_TABS);
        assertTrue(offered.get().USE_TAB_CHARACTER);
        assertTrue(offered.get().SMART_TABS);
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_TAB);
        assertEquals("def main():\n      value = 1", myFixture.getEditor().getDocument().getText());
    }

    public void testIndentProviderClearsGuardAfterExceptionAndIgnoresOtherFiles() {
        var failNext = new AtomicBoolean();
        var calls = new AtomicInteger();
        var provider = new NagiIndentOptionsProvider();
        FileIndentOptionsProvider perFile = new FileIndentOptionsProvider() {
            @Override public boolean useOnFullReformat() { return true; }
            @Override public CommonCodeStyleSettings.IndentOptions getIndentOptions(Project project,
                                                                                     CodeStyleSettings settings, VirtualFile file) {
                if (file.getFileType() != NagiFileType.HIGH) return null;
                calls.incrementAndGet();
                if (failNext.getAndSet(false)) throw new IllegalStateException("synthetic code-style failure");
                return (CommonCodeStyleSettings.IndentOptions)
                        settings.getLanguageIndentOptions(NagiLanguage.HIGH).clone();
            }
        };
        ExtensionTestUtil.maskExtensions(FileIndentOptionsProvider.EP_NAME, List.of(provider, perFile), getTestRootDisposable());
        var file = myFixture.configureByText("main.nagi", "print(1)<caret>");
        var settings = CodeStyle.getSettings(file);
        calls.set(0);
        failNext.set(true);
        try {
            provider.getIndentOptions(getProject(), settings, file.getVirtualFile());
            fail("Expected the next provider's exception");
        } catch (IllegalStateException expected) {
            assertEquals("synthetic code-style failure", expected.getMessage());
        }
        assertNotNull(provider.getIndentOptions(getProject(), settings, file.getVirtualFile()));
        assertEquals(2, calls.get());
        var other = myFixture.configureByText("notes.txt", "plain text");
        assertNull(provider.getIndentOptions(getProject(), CodeStyle.getSettings(other), other.getVirtualFile()));
        assertEquals(2, calls.get());
    }

    public void testPasteKeepsSpacesInHighBlock() {
        myFixture.configureByText("main.nagi", "def main():\n    <caret>");
        CopyPasteManager.getInstance().setContents(new StringSelection("value = 1\nvalue = 2"));
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_PASTE);
        myFixture.checkResult("def main():\n    value = 1\n    value = 2<caret>");
    }

    public void testReformatKeepsBasicIndent() {
        myFixture.configureByText("main.nagi", "def main():\n    value = 1<caret>\n");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_REFORMAT);
        myFixture.checkResult("def main():\n    value = 1<caret>\n");
    }

    public void testMulticaretEnterKeepsBothEdits() {
        myFixture.configureByText("main.nagi", "value = 1<caret>\nother = 2");
        var editor = myFixture.getEditor();
        Caret second = editor.getCaretModel().addCaret(editor.offsetToVisualPosition(editor.getDocument().getTextLength()));
        assertNotNull(second);
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        assertEquals("value = 1\n\nother = 2\n", editor.getDocument().getText());
        assertEquals(2, editor.getCaretModel().getCaretCount());
    }

    public void testCrLfAndIncompleteStringEnter() {
        myFixture.configureByText("main.nagi", "def main():\r\n    print(\"猫:<caret>");
        var editor = myFixture.getEditor();
        int before = editor.getDocument().getLineCount();
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        assertEquals(before + 1, editor.getDocument().getLineCount());
        assertFalse(editor.getDocument().getText().contains("\r\r"));
        assertTrue(editor.getDocument().getText().endsWith("    "));
    }

    public void testRepeatedEnterAndUndoRedo() {
        myFixture.configureByText("main.nagi", "def main():<caret>");
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.performEditorAction(IdeActions.ACTION_EDITOR_ENTER);
        myFixture.checkResult("def main():\n    \n    <caret>");
        myFixture.performEditorAction(IdeActions.ACTION_UNDO);
        myFixture.checkResult("def main():\n    <caret>");
        myFixture.performEditorAction(IdeActions.ACTION_REDO);
        myFixture.checkResult("def main():\n    \n    <caret>");
    }

    @FunctionalInterface
    private interface PhysicalFileCase { void run(VirtualFile file) throws Exception; }

    private void rename(VirtualFile file, String name) throws IOException {
        WriteCommandAction.runWriteCommandAction(getProject(), (ThrowableComputable<Void, IOException>) () -> {
            file.rename(this, name);
            return null;
        });
    }

    private void withPhysicalFile(String name, String text, String editorConfig,
                                  PhysicalFileCase test) throws Exception {
        Path base = Path.of(getProject().getBasePath());
        Files.createDirectories(base);
        Path root = Files.createTempDirectory(base, "nagi input file ");
        Path config = root.resolve(".editorconfig");
        VirtualFile virtualFile = null;
        try {
            Files.writeString(config, editorConfig);
            Path path = root.resolve(name);
            Files.writeString(path, text);
            assertNotNull(LocalFileSystem.getInstance().refreshAndFindFileByNioFile(config));
            virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
            assertNotNull(virtualFile);
            test.run(virtualFile);
        } finally {
            if (virtualFile != null) FileEditorManager.getInstance(getProject()).closeFile(virtualFile);
            try (var files = Files.walk(root)) {
                for (Path path : files.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }

    private void withHighIndentOptions(Consumer<CommonCodeStyleSettings.IndentOptions> configure, Runnable test) {
        var settings = CodeStyle.getSettings(getProject()).clone();
        configure.accept(settings.getLanguageIndentOptions(NagiLanguage.HIGH));
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
