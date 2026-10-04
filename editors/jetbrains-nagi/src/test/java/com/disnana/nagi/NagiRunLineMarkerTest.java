package com.disnana.nagi;

import com.intellij.codeInsight.daemon.LineMarkerProviders;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.List;
import java.nio.file.Files;
import java.nio.file.Path;

/** Exercise the registered provider against real PSI and editor gutter rendering. */
public class NagiRunLineMarkerTest extends BasePlatformTestCase {
    private int sample;
    private PsiFile configureSource(String name, String source) {
        var file = myFixture.addFileToProject("case" + sample++ + "/" + name, source);
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        return file;
    }
    public void testHighEntryPointsHaveGutters() {
        for (String declaration : List.of("def main():", "async def main():", "def main() -> unit:",
                "async def main() -> Result[unit, str]:", "def main(\n):", "def main() -> fn[i64, i64]:")) {
            configureSource("main.nagi", declaration + "\n    print(1)\n");
            assertEquals(declaration, 1, myFixture.findAllGutters().size());
            assertEquals("Run Nagi main (nearest project or this file)", myFixture.findAllGutters().getFirst().getTooltipText());
        }
    }
    public void testLowEntryPointsHaveGutters() {
        for (String declaration : List.of("fn main() {", "async fn main() {", "fn main() -> unit {",
                "  fn main()\n{", "fn main(\n) -> Result[unit, str] {", "fn main() -> fn[i64, i64] {")) {
            configureSource("main.low", declaration + "\n    print(1);\n}\n");
            assertEquals(declaration, 1, myFixture.findAllGutters().size());
        }
    }
    public void testFakeNestedAndIncompleteHighMainHaveNoGutters() {
        for (String source : List.of("# def main():\n", "text = \"def main():\"\n", "def helper():\n    print(1)\n",
                "class App:\n    def main():\n        print(1)\n", "def outer():\n    def main():\n        print(1)\n",
                "def main(value: i64):\n    print(value)\n", "def main", "def main(", "def main()", "async\ndef main():\n",
                "def main() -> str\nother: i64\n", "def main() ->\ndef helper():\n    print(1)\n",
                "values = [\ndef main():\n]\n")) {
            configureSource("fake.nagi", source);
            assertEmpty(source, myFixture.findAllGutters());
        }
    }
    public void testFakeNestedAndIncompleteLowMainHaveNoGutters() {
        for (String source : List.of("# fn main() {}\n", "let text = \"fn main() {}\";", "fn helper() {}",
                "fn outer() { fn main() {} }", "record App { fn main() {} }", "extern fn main() -> unit;",
                "fn main(value: i64) {}", "fn main", "fn main(", "fn main()", "fn main() ->",
                "let callback = fn main() {}", "print(fn main() {});", "fn main() -> fn other() {}")) {
            configureSource("fake.low", source);
            assertEmpty(source, myFixture.findAllGutters());
        }
    }
    public void testLowCommentsAndStringsDoNotChangeTopLevel() {
        configureSource("main.low", "fn helper() { print(\"}\"); # }\n}\nfn main() { print(1); }\n");
        assertEquals(1, myFixture.findAllGutters().size());
    }
    public void testMarkerTargetsItsFileEvenAfterAnotherEditorOpens() {
        var main = configureSource("main.nagi", "def main():\n    print(1)\n");
        var name = main.findElementAt(main.getText().indexOf("main"));
        assertNotNull(name);
        var marker = new NagiRunLineMarker().getLineMarkerInfo(name);
        assertNotNull(marker);
        assertNotNull(marker.getNavigationHandler());
        configureSource("other.py", "print('different run configuration')\n");
        var target = marker.getElement().getContainingFile();
        assertSame(main.getVirtualFile(), target.getVirtualFile());
        assertSame(getProject(), target.getProject());
        assertSame(main, target);
    }
    public void testEntryPointCacheIsInvalidatedWhenDeclarationChanges() {
        configureSource("main.nagi", "def main():\n    print(1)\n");
        assertEquals(1, myFixture.findAllGutters().size());
        var document = myFixture.getEditor().getDocument();
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(4, 8, "helper"));
        PsiDocumentManager.getInstance(getProject()).commitDocument(document);
        assertEmpty(myFixture.findAllGutters());
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(4, 10, "main"));
        PsiDocumentManager.getInstance(getProject()).commitDocument(document);
        assertEquals(1, myFixture.findAllGutters().size());
    }
    public void testProviderIsRegisteredForBothLanguagesOnly() {
        assertTrue(LineMarkerProviders.getInstance().allForLanguage(NagiLanguage.HIGH).stream()
                .anyMatch(provider -> provider instanceof NagiRunLineMarker));
        assertTrue(LineMarkerProviders.getInstance().allForLanguage(NagiLanguage.LOW).stream()
                .anyMatch(provider -> provider instanceof NagiRunLineMarker));
        myFixture.configureByText("main.txt", "def main():\n    print(1)\n");
        assertEmpty(myFixture.findAllGutters());
    }
    public void testVirtualPreviewCannotInvokeRun() {
        var preview = com.intellij.psi.PsiFileFactory.getInstance(getProject()).createFileFromText(
                "preview.nagi", NagiFileType.HIGH, "def main():\n    print(1)\n");
        assertFalse(NagiCompilerAction.isSourceFile(preview.getVirtualFile()));
        var name = preview.findElementAt(preview.getText().indexOf("main"));
        assertNotNull(name);
        assertNull(new NagiRunLineMarker().getLineMarkerInfo(name));
        new NagiCompilerAction.Run().execute(preview.getProject(), preview.getVirtualFile());
        // Must return before trust dialogs, saves, or process startup.
    }
    public void testHighGutterClickInvokesTheCompilerWithItsSavedFile() throws Exception { invokeRun(false, false); }
    public void testLowGutterClickInvokesTheCompilerWithItsSavedFile() throws Exception { invokeRun(true, false); }
    public void testGutterClickKeepsNearestProjectSelection() throws Exception { invokeRun(false, true); }

    private void invokeRun(boolean low, boolean projectMode) throws Exception {
        org.junit.Assume.assumeFalse("The recorder uses a POSIX executable", com.intellij.openapi.util.SystemInfo.isWindows);
        Path temporary = Files.createTempDirectory("nagi gutter run ");
        var settings = NagiSettings.getInstance().getState();
        String previousCompiler = settings.compilerPath;
        var editorsBeforeRun = java.util.Set.of(com.intellij.openapi.editor.EditorFactory.getInstance().getAllEditors());
        try {
            Path captured = temporary.resolve("arguments.txt");
            Path compiler = temporary.resolve("nagic");
            Files.writeString(compiler, "#!/bin/sh\nprintf '%s\\n' \"$@\" > '" + captured.toString().replace("'", "'\"'\"'") + "'\n");
            assertTrue(compiler.toFile().setExecutable(true));
            settings.compilerPath = compiler.toString();
            com.intellij.ide.impl.TrustedProjects.setTrusted(getProject(), true);
            Path source = temporary.resolve(low ? "main.low" : "main.nagi");
            Files.writeString(source, low ? "fn main() { print(1); }\n" : "def main():\n    print(1)\n");
            var virtual = com.intellij.openapi.vfs.LocalFileSystem.getInstance().refreshAndFindFileByNioFile(source);
            assertNotNull(virtual);
            myFixture.configureFromExistingVirtualFile(virtual);
            var file = myFixture.getFile();
            Path manifest = source.getParent().resolve("nagi.toml");
            if (projectMode) Files.writeString(manifest, "entry = \"main.nagi\"\n");
            var name = file.findElementAt(file.getText().indexOf("main"));
            assertNotNull(name);
            var marker = new NagiRunLineMarker().getLineMarkerInfo(name);
            assertNotNull(marker);
            var document = myFixture.getEditor().getDocument();
            WriteCommandAction.runWriteCommandAction(getProject(), () -> document.insertString(document.getTextLength(), "# saved by gutter action\n"));
            configureSource("other.py", "print('not the gutter target')\n");
            marker.getNavigationHandler().navigate(null, marker.getElement());
            com.intellij.testFramework.PlatformTestUtil.waitWithEventsDispatching("Nagi gutter did not start the compiler", () -> {
                try { return Files.exists(captured) && Files.size(captured) > 0; }
                catch (java.io.IOException ignored) { return false; }
            }, 5);
            var arguments = Files.readAllLines(captured);
            assertEquals("run", arguments.getFirst());
            if (projectMode) {
                assertEquals("--project", arguments.get(1));
                assertEquals(manifest.toString(), arguments.get(2));
                assertEquals("--out", arguments.get(3));
            } else {
                assertEquals(source.toString(), arguments.get(1));
                assertEquals("--out", arguments.get(2));
            }
            assertTrue(Files.readString(source).endsWith("# saved by gutter action\n"));
            com.intellij.testFramework.PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();
            com.intellij.testFramework.PlatformTestUtil.dispatchAllInvocationEventsInIdeEventQueue();
        } finally {
            settings.compilerPath = previousCompiler;
            var contents = com.intellij.execution.ui.RunContentManager.getInstance(getProject());
            for (var descriptor : List.copyOf(contents.getAllDescriptors())) {
                contents.removeRunContent(com.intellij.execution.executors.DefaultRunExecutor.getRunExecutorInstance(), descriptor);
                if (!com.intellij.openapi.util.Disposer.isDisposed(descriptor)) com.intellij.openapi.util.Disposer.dispose(descriptor);
            }
            com.intellij.testFramework.PlatformTestUtil.dispatchAllEventsInIdeEventQueue();
            // LightPlatform's headless Run content manager may not retain the
            // descriptor. Release only console viewers created by this click.
            var factory = com.intellij.openapi.editor.EditorFactory.getInstance();
            for (var editor : factory.getAllEditors()) {
                if (editor.isViewer() && !editorsBeforeRun.contains(editor)) factory.releaseEditor(editor);
            }
            try (var paths = Files.walk(temporary)) {
                for (var path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }
}
