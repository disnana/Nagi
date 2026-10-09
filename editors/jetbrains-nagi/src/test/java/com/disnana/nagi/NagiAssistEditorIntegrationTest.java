package com.disnana.nagi;

import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.util.ui.UIUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;

/** Registered editor features exercised against the matching real compiler. */
public class NagiAssistEditorIntegrationTest extends BasePlatformTestCase {
    private Path root;
    private String originalCompiler;
    private int originalTimeout;
    private boolean originalTrust;
    @Override protected void setUp() throws Exception {
        super.setUp();
        String compiler = System.getenv("NAGI_TEST_COMPILER");
        org.junit.Assume.assumeTrue("matching compiler required", compiler != null && !compiler.isBlank());
        com.intellij.openapi.vfs.newvfs.impl.VfsRootAccess.allowRootAccess(getTestRootDisposable(), Path.of(compiler).getParent().toString());
        Path base = Path.of(getProject().getBasePath());
        Files.createDirectories(base);
        root = Files.createTempDirectory(base, "nagi assistance integration ");
        originalCompiler = NagiSettings.getInstance().getState().compilerPath;
        originalTimeout = NagiSettings.getInstance().getState().checkTimeoutSeconds;
        originalTrust = TrustedProjects.isProjectTrusted(getProject());
        NagiSettings.update(compiler, 15);
        TrustedProjects.setProjectTrusted(getProject(), true);
    }
    @Override protected void tearDown() throws Exception {
        try {
            if (root != null) {
                NagiSettings.update(originalCompiler, originalTimeout);
                TrustedProjects.setProjectTrusted(getProject(), originalTrust);
                try (var files = Files.walk(root)) {
                    for (Path file : files.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
                }
            }
        } finally { super.tearDown(); }
    }
    private void open(String name, String text) throws Exception {
        Path path = root.resolve(name);
        Files.writeString(path, text);
        var file = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(file);
        myFixture.configureFromExistingVirtualFile(file);
    }
    private void waitFor(BooleanSupplier done) throws Exception {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
        while (!done.getAsBoolean() && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents();
            Thread.sleep(5);
        }
        var service = getProject().getService(NagiAssistService.class);
        if (!done.getAsBoolean()) {
            Thread.getAllStackTraces().forEach((thread, stack) -> {
                if (thread.getName().contains("Nagi")) {
                    System.err.println(thread.getName());
                    for (var frame : stack) System.err.println(frame);
                }
            });
        }
        assertTrue(service.state() + ": " + service.statusMessage(), done.getAsBoolean());
    }
    public void testRegisteredCompletionAndDiagnosticsUseUnsavedCompilerInput() throws Exception {
        open("main.nagi", "def main():\n    saved = True\n");
        var document = myFixture.getEditor().getDocument();
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.setText("def main():\n    fresh = True\n    fr\n"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        myFixture.getEditor().getCaretModel().moveToOffset(document.getTextLength() - 1);
        var service = getProject().getService(NagiAssistService.class);
        service.requestCompletion(myFixture.getFile(), myFixture.getEditor());
        waitFor(() -> service.fresh(myFixture.getFile(), myFixture.getEditor()) != null);
        var variants = myFixture.completeBasic();
        assertNotNull(variants);
        assertTrue(java.util.Arrays.stream(variants).anyMatch(item -> item.getLookupString().equals("fresh")));
        assertFalse(java.util.Arrays.stream(variants).anyMatch(item -> item.getLookupString().equals("saved")));
        assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));
        var response = service.fresh(myFixture.getFile());
        assertNotNull(response);
        var diagnostic = response.response().diagnostics().getFirst();
        assertEquals(Integer.valueOf(3), diagnostic.line());
        assertTrue(myFixture.doHighlighting().stream().anyMatch(info ->
                info.getDescription() != null && info.getDescription().contains(diagnostic.message())));
    }
    public void testRegisteredReferencesNavigateHighLowAndImportedFiles() throws Exception {
        for (boolean low : new boolean[]{false, true}) {
            String name = low ? "main.low" : "main.nagi";
            String text = low ? "fn main() { let value = 1; print(value); }\n" : "def main():\n    value = 1\n    print(value)\n";
            open(name, text);
            var file = myFixture.getFile();
            var service = getProject().getService(NagiAssistService.class);
            service.requestBaseline(file);
            waitFor(() -> service.fresh(file) != null);
            var reference = file.findReferenceAt(text.lastIndexOf("value"));
            assertNotNull(name, reference);
            var target = reference.resolve();
            assertNotNull(name, target);
            assertEquals(text.indexOf("value"), target.getTextOffset());
        }
        Files.writeString(root.resolve("helper.nagi"), "def helper() -> i64:\n    return 1\n");
        open("imports.nagi", "import \"helper.nagi\"\ndef main():\n    print(helper())\n");
        var file = myFixture.getFile();
        var service = getProject().getService(NagiAssistService.class);
        service.requestBaseline(file);
        waitFor(() -> service.fresh(file) != null);
        var reference = file.findReferenceAt(file.getText().lastIndexOf("helper"));
        assertNotNull(reference);
        assertEquals("helper.nagi", reference.resolve().getContainingFile().getName());
        var importReference = file.findReferenceAt(file.getText().indexOf("helper.nagi"));
        assertNotNull(importReference);
        assertEquals("helper.nagi", importReference.resolve().getContainingFile().getName());
    }
}
