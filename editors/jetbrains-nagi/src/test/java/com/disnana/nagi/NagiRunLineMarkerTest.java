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
    public void testUntrustedProjectCannotInvokeCompiler() throws Exception {
        org.junit.Assume.assumeFalse("The recorder uses a POSIX executable", com.intellij.openapi.util.SystemInfo.isWindows);
        Path temporary = Files.createTempDirectory("nagi untrusted gutter ");
        var settings = NagiSettings.getInstance().getState();
        String previousCompiler = settings.compilerPath;
        boolean previousTrust = com.intellij.ide.trustedProjects.TrustedProjects.isProjectTrusted(getProject());
        var previousDialog = com.intellij.openapi.ui.TestDialogManager.getTestImplementation();
        try {
            Path captured = temporary.resolve("arguments.txt");
            Path compiler = temporary.resolve("nagic");
            Files.writeString(compiler, "#!/bin/sh\nprintf '%s\\n' \"$@\" > '" + captured.toString().replace("'", "'\"'\"'") + "'\n");
            assertTrue(compiler.toFile().setExecutable(true));
            settings.compilerPath = compiler.toString();
            com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), false);
            com.intellij.openapi.ui.TestDialogManager.setTestDialog(com.intellij.openapi.ui.TestDialog.OK);
            var file = configureSource("untrusted.nagi", "def main():\n    print(1)\n");
            var document = myFixture.getEditor().getDocument();
            WriteCommandAction.runWriteCommandAction(getProject(), () -> document.insertString(document.getTextLength(), "# unsaved\n"));
            assertTrue(com.intellij.openapi.fileEditor.FileDocumentManager.getInstance().isDocumentUnsaved(document));

            assertTrue("the untrusted fixture must be an eligible local Nagi source",
                    NagiCompilerAction.isSourceFile(file.getVirtualFile()));
            assertFalse("the fixture must exercise actual untrusted-project handling",
                    com.intellij.ide.trustedProjects.TrustedProjects.isProjectTrusted(getProject()));
            var denialMessages = new java.util.ArrayList<String>();
            com.intellij.openapi.ui.TestDialogManager.setTestDialog(message -> {
                denialMessages.add(message);
                return com.intellij.openapi.ui.TestDialog.OK.show(message);
            });
            new NagiCompilerAction.Run().execute(getProject(), file.getVirtualFile());
            assertEquals(List.of("Trust this project before executing the Nagi compiler."), denialMessages);
            assertTrue("untrusted action must not save input before returning",
                    com.intellij.openapi.fileEditor.FileDocumentManager.getInstance().isDocumentUnsaved(document));
            com.intellij.testFramework.PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();

            assertFalse("untrusted project started the compiler", Files.exists(captured));
            assertTrue("untrusted project input should remain unsaved",
                    com.intellij.openapi.fileEditor.FileDocumentManager.getInstance().isDocumentUnsaved(document));
        } finally {
            com.intellij.openapi.ui.TestDialogManager.setTestDialog(previousDialog);
            com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), previousTrust);
            settings.compilerPath = previousCompiler;
            try (var paths = Files.walk(temporary)) {
                for (var path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }
    public void testHighGutterClickInvokesTheCompilerWithItsSavedFile() throws Exception { invokeRun(false, false); }
    public void testLowGutterClickInvokesTheCompilerWithItsSavedFile() throws Exception { invokeRun(true, false); }
    public void testGutterClickKeepsNearestProjectSelection() throws Exception { invokeRun(false, true); }
    public void testBackgroundBarrierPreventsAnEarlyCompilerStart() throws Exception { exerciseControlledBarrier(false, false); }
    public void testFailedBarrierDoesNotStartTheCompiler() throws Exception { exerciseControlledBarrier(true, false); }
    public void testTrustRevokedDuringBarrierDoesNotStartTheCompiler() throws Exception { exerciseControlledBarrier(false, true); }

    private void exerciseControlledBarrier(boolean fail, boolean revokeTrust) throws Exception {
        org.junit.Assume.assumeFalse("The recorder uses a POSIX executable", com.intellij.openapi.util.SystemInfo.isWindows);
        Path temporary = Files.createTempDirectory("nagi controlled barrier ");
        var settings = NagiSettings.getInstance().getState();
        String previousCompiler = settings.compilerPath;
        boolean previousTrust = com.intellij.ide.trustedProjects.TrustedProjects.isProjectTrusted(getProject());
        var previousDialog = com.intellij.openapi.ui.TestDialogManager.getTestImplementation();
        var editorsBeforeRun = java.util.Set.of(com.intellij.openapi.editor.EditorFactory.getInstance().getAllEditors());
        var entered = new java.util.concurrent.CountDownLatch(1);
        var release = new java.util.concurrent.CountDownLatch(1);
        var errors = new java.util.concurrent.CopyOnWriteArrayList<String>();
        var ranOnEdt = new java.util.concurrent.atomic.AtomicBoolean(true);
        var flushCompleted = new java.util.concurrent.atomic.AtomicBoolean(false);
        String asyncProperty = "intellij.progress.task.ignoreHeadless";
        String previousAsyncProperty = System.getProperty(asyncProperty);
        try {
            // Headless IntelliJ tests otherwise run Backgroundable.queue() synchronously.
            System.setProperty(asyncProperty, "true");
            Path captured = temporary.resolve("started.txt");
            Path compiler = temporary.resolve("nagic");
            Files.writeString(compiler, "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$$\" > "
                    + shellQuote(temporary.resolve("started.tmp")) + "\nmv "
                    + shellQuote(temporary.resolve("started.tmp")) + " " + shellQuote(captured) + "\n");
            assertTrue(compiler.toFile().setExecutable(true));
            settings.compilerPath = compiler.toString();
            com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), true);
            com.intellij.openapi.ui.TestDialogManager.setTestDialog(message -> {
                errors.add(message);
                return com.intellij.openapi.ui.TestDialog.OK.show(message);
            });
            Path source = Files.writeString(temporary.resolve("main.nagi"), "def main():\n    print(1)\n");
            var virtual = com.intellij.openapi.vfs.LocalFileSystem.getInstance().refreshAndFindFileByNioFile(source);
            assertNotNull(virtual);
            myFixture.configureFromExistingVirtualFile(virtual);
            var document = myFixture.getEditor().getDocument();
            WriteCommandAction.runWriteCommandAction(getProject(),
                    () -> document.insertString(document.getTextLength(), "# save before external run\n"));
            new NagiCompilerAction("run") {
                @Override void flushInputs() throws java.io.IOException {
                    ranOnEdt.set(com.intellij.openapi.application.ApplicationManager.getApplication().isDispatchThread());
                    entered.countDown();
                    try {
                        if (!release.await(10, java.util.concurrent.TimeUnit.SECONDS)) {
                            throw new java.io.IOException("test barrier was not released");
                        }
                    } catch (InterruptedException interrupted) {
                        Thread.currentThread().interrupt();
                        throw new java.io.IOException("test barrier was interrupted", interrupted);
                    }
                    if (fail) throw new java.io.IOException("injected disk write failure");
                    NagiSaveBarrier.awaitDiskWrites();
                    flushCompleted.set(true);
                }
            }.execute(getProject(), virtual);
            com.intellij.testFramework.PlatformTestUtil.waitWithEventsDispatching(
                    "Nagi action did not reach its save barrier", () -> entered.getCount() == 0, 5);
            assertFalse("save barrier must run off the EDT", ranOnEdt.get());
            assertFalse("compiler started while disk writes were still pending", Files.exists(captured));
            assertTrue("error reported before save barrier completed", errors.isEmpty());
            if (revokeTrust) com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), false);
            release.countDown();
            if (!fail && !revokeTrust) {
                com.intellij.testFramework.PlatformTestUtil.waitWithEventsDispatching(
                        "compiler did not start after the save barrier", () -> Files.exists(captured), 5);
                long recorderPid = Long.parseLong(Files.readString(captured).trim());
                var recorder = ProcessHandle.of(recorderPid);
                if (recorder.isPresent()) recorder.get().onExit().get(5, java.util.concurrent.TimeUnit.SECONDS);
            } else if (fail) {
                com.intellij.testFramework.PlatformTestUtil.waitWithEventsDispatching(
                        "save failure was not reported", () -> !errors.isEmpty(), 5);
                assertTrue(errors.getFirst().contains("Could not finish saving Nagi inputs"));
                assertTrue(errors.getFirst().contains("injected disk write failure"));
            }
            com.intellij.testFramework.PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();
            if (fail) assertFalse("failed flush was marked complete", flushCompleted.get());
            else assertTrue("save-to-disk flush did not complete", flushCompleted.get());
            if (revokeTrust) assertTrue("trust refusal was obscured by a save error", errors.isEmpty());
            if (fail || revokeTrust) assertFalse("compiler started despite failed preparation", Files.exists(captured));
        } finally {
            release.countDown();
            com.intellij.testFramework.PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();
            if (previousAsyncProperty == null) System.clearProperty(asyncProperty);
            else System.setProperty(asyncProperty, previousAsyncProperty);
            com.intellij.openapi.ui.TestDialogManager.setTestDialog(previousDialog);
            com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), previousTrust);
            settings.compilerPath = previousCompiler;
            var contents = com.intellij.execution.ui.RunContentManager.getInstance(getProject());
            for (var descriptor : List.copyOf(contents.getAllDescriptors())) {
                contents.removeRunContent(com.intellij.execution.executors.DefaultRunExecutor.getRunExecutorInstance(), descriptor);
                if (!com.intellij.openapi.util.Disposer.isDisposed(descriptor)) com.intellij.openapi.util.Disposer.dispose(descriptor);
            }
            com.intellij.testFramework.PlatformTestUtil.dispatchAllEventsInIdeEventQueue();
            var factory = com.intellij.openapi.editor.EditorFactory.getInstance();
            for (var editor : factory.getAllEditors()) {
                if (editor.isViewer() && !editorsBeforeRun.contains(editor)) factory.releaseEditor(editor);
            }
            flushBeforeDeletingSources();
            try (var paths = Files.walk(temporary)) {
                for (var path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }

    private void invokeRun(boolean low, boolean projectMode) throws Exception {
        org.junit.Assume.assumeFalse("The recorder uses a POSIX executable", com.intellij.openapi.util.SystemInfo.isWindows);
        Path temporary = Files.createTempDirectory("nagi gutter run ");
        var settings = NagiSettings.getInstance().getState();
        String previousCompiler = settings.compilerPath;
        boolean previousTrust = com.intellij.ide.trustedProjects.TrustedProjects.isProjectTrusted(getProject());
        var editorsBeforeRun = java.util.Set.of(com.intellij.openapi.editor.EditorFactory.getInstance().getAllEditors());
        try {
            Path captured = temporary.resolve("arguments.txt");
            Path snapshot = temporary.resolve("source-at-start.txt");
            Path completed = temporary.resolve("completed.txt");
            Path compiler = temporary.resolve("nagic");
            com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), true);
            Path source = temporary.resolve(low ? "main.low" : "main.nagi");
            String initial = low ? "fn main() { print(1); }\n" : "def main():\n    print(1)\n";
            String expected = initial + "# saved by gutter action\n";
            Files.writeString(source, initial);
            Files.writeString(compiler, "#!/bin/sh\nset -eu\n"
                    + "printf '%s\\n' \"$@\" > " + shellQuote(temporary.resolve("arguments.tmp")) + "\n"
                    + "cat " + shellQuote(source) + " > " + shellQuote(temporary.resolve("source.tmp")) + "\n"
                    + "mv " + shellQuote(temporary.resolve("arguments.tmp")) + " " + shellQuote(captured) + "\n"
                    + "mv " + shellQuote(temporary.resolve("source.tmp")) + " " + shellQuote(snapshot) + "\n"
                    + "printf '%s\\n' \"$$\" > " + shellQuote(temporary.resolve("completed.tmp")) + "\n"
                    + "mv " + shellQuote(temporary.resolve("completed.tmp")) + " " + shellQuote(completed) + "\n");
            assertTrue(compiler.toFile().setExecutable(true));
            settings.compilerPath = compiler.toString();
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
            assertEquals(expected, document.getText());
            marker.getNavigationHandler().navigate(null, marker.getElement());
            com.intellij.testFramework.PlatformTestUtil.waitWithEventsDispatching("Nagi gutter did not start the compiler", () -> {
                return Files.exists(completed);
            }, 5);
            long recorderPid = Long.parseLong(Files.readString(completed).trim());
            var recorder = ProcessHandle.of(recorderPid);
            if (recorder.isPresent()) recorder.get().onExit().get(5, java.util.concurrent.TimeUnit.SECONDS);
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
            assertEquals("compiler must read the edited bytes at process start", expected, Files.readString(snapshot));
            assertEquals(expected, Files.readString(source));
            com.intellij.testFramework.PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();
            com.intellij.testFramework.PlatformTestUtil.dispatchAllInvocationEventsInIdeEventQueue();
        } finally {
            settings.compilerPath = previousCompiler;
            com.intellij.ide.trustedProjects.TrustedProjects.setProjectTrusted(getProject(), previousTrust);
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
            flushBeforeDeletingSources();
            try (var paths = Files.walk(temporary)) {
                for (var path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }

    private static String shellQuote(Path path) {
        return "'" + path.toString().replace("'", "'\"'\"'") + "'";
    }

    private static void flushBeforeDeletingSources() throws Exception {
        var flushed = java.util.concurrent.CompletableFuture.runAsync(() -> {
            try {
                NagiSaveBarrier.awaitDiskWrites();
            } catch (java.io.IOException failure) {
                throw new java.util.concurrent.CompletionException(failure);
            }
        });
        com.intellij.testFramework.PlatformTestUtil.waitWithEventsDispatching(
                "pending IDE file writes did not finish", flushed::isDone, 5);
        flushed.get();
    }
}
