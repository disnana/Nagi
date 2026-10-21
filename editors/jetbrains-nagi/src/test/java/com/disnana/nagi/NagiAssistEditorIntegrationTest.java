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
        // A newly discovered import emits a VFS event and invalidates the first
        // request. Model the editor's subsequent navigation/annotator queries.
        waitFor(() -> { service.requestNavigation(file); return service.fresh(file) != null; });
        var reference = file.findReferenceAt(file.getText().lastIndexOf("helper"));
        assertNotNull(reference);
        assertEquals("helper.nagi", reference.resolve().getContainingFile().getName());
        var importReference = file.findReferenceAt(file.getText().indexOf("helper.nagi"));
        assertNotNull(importReference);
        assertEquals("helper.nagi", importReference.resolve().getContainingFile().getName());
    }

    public void testNamedImportNavigationTargetsSourceOriginalNameAliasAndUses() throws Exception {
        Files.writeString(root.resolve("helper.nagi"),
                "def make() -> i64:\n    return 1\n");
        String text = "import helper as tools\n"
                + "from helper import make as make_alias\n"
                + "def main():\n"
                + "    print(tools.make())\n"
                + "    print(make_alias())\n";
        open("named-imports.nagi", text);
        var file = myFixture.getFile();
        var service = getProject().getService(NagiAssistService.class);
        service.requestBaseline(file);
        waitFor(() -> service.fresh(file) != null);

        int firstModule = text.indexOf("helper");
        int secondModule = text.indexOf("helper", firstModule + 1);
        int originalName = text.indexOf("make", secondModule + "helper".length());
        int alias = text.indexOf("make_alias");
        int moduleUse = text.indexOf("tools.make");
        int aliasUse = text.lastIndexOf("make_alias");
        var sourceReference = file.findReferenceAt(firstModule);
        assertNotNull("import module token has a compiler reference", sourceReference);
        assertEquals("helper.nagi", sourceReference.resolve().getContainingFile().getName());
        var fromReference = file.findReferenceAt(secondModule);
        assertNotNull("from source token has a compiler reference", fromReference);
        assertEquals("helper.nagi", fromReference.resolve().getContainingFile().getName());
        var originalReference = file.findReferenceAt(originalName);
        assertNotNull("selected original name has a compiler reference", originalReference);
        assertEquals("helper.nagi", originalReference.resolve().getContainingFile().getName());
        var aliasReference = file.findReferenceAt(alias);
        assertNotNull("selected alias has a compiler reference", aliasReference);
        assertEquals("helper.nagi", aliasReference.resolve().getContainingFile().getName());
        var moduleUseReference = file.findReferenceAt(moduleUse);
        assertNotNull("module use has compiler navigation facts", moduleUseReference);
        assertNotNull("module use resolves through compiler binding metadata", moduleUseReference.resolve());
        var aliasUseReference = file.findReferenceAt(aliasUse);
        assertNotNull("alias use has compiler navigation facts", aliasUseReference);
        assertEquals("helper.nagi", aliasUseReference.resolve().getContainingFile().getName());
    }

    public void testImportSourceAndFromNameCompletionUseCompilerContextAndSuppressCallParentheses() throws Exception {
        Files.writeString(root.resolve("helper.nagi"),
                "def build() -> i64:\n    return 1\n");
        open("source-completion.nagi", "import hel\n");
        var sourceFile = myFixture.getFile();
        var sourceEditor = myFixture.getEditor();
        sourceEditor.getCaretModel().moveToOffset(sourceFile.getTextLength() - 1);
        var service = getProject().getService(NagiAssistService.class);
        service.requestCompletion(sourceFile, sourceEditor);
        waitFor(() -> service.fresh(sourceFile, sourceEditor) != null);
        assertEquals("import-source", service.fresh(sourceFile, sourceEditor).response().importContext().kind());
        var sourceVariants = myFixture.completeBasic();
        assertNotNull(sourceVariants);
        assertTrue(java.util.Arrays.stream(sourceVariants).anyMatch(item -> item.getLookupString().equals("helper")));

        open("name-completion.nagi", "from helper import bu\n");
        var nameFile = myFixture.getFile();
        var nameEditor = myFixture.getEditor();
        nameEditor.getCaretModel().moveToOffset(nameFile.getTextLength() - 1);
        service.requestCompletion(nameFile, nameEditor);
        waitFor(() -> service.fresh(nameFile, nameEditor) != null);
        var response = service.fresh(nameFile, nameEditor).response();
        assertEquals("import-names", response.importContext().kind());
        assertTrue(response.completions().stream().anyMatch(item -> item.name().equals("build") && item.kind().equals("function")));
        var variants = myFixture.completeBasic();
        assertNotNull(variants);
        var build = java.util.Arrays.stream(variants).filter(item -> item.getLookupString().equals("build")).findFirst().orElse(null);
        assertNotNull(build);
        myFixture.getLookup().setCurrentItem(build);
        myFixture.finishLookup('(');
        assertEquals("from helper import build\n", nameEditor.getDocument().getText());
    }

    public void testNamedImportCarriesDiagnosticsFromImportedSource() throws Exception {
        Path helperPath = Files.writeString(root.resolve("broken_helper.nagi"),
                "def broken() -> i64:\n    return \"wrong\"\n");
        var helperVirtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(helperPath);
        assertNotNull(helperVirtualFile);
        open("main.nagi", "import broken_helper\ndef main():\n    print(1)\n");
        var main = myFixture.getFile();
        var service = getProject().getService(NagiAssistService.class);
        service.requestBaseline(main);
        waitFor(() -> service.fresh(main) != null);
        var current = service.fresh(main);
        assertTrue(current.response().sourceFiles().stream().anyMatch(path -> path.endsWith("broken_helper.nagi")));
        var importedDiagnostic = current.response().diagnostics().stream()
                .filter(diagnostic -> diagnostic.file() != null && diagnostic.file().endsWith("broken_helper.nagi"))
                .findFirst().orElseThrow();
        var imported = com.intellij.psi.PsiManager.getInstance(getProject()).findFile(helperVirtualFile);
        assertNotNull(imported);
        myFixture.configureFromExistingVirtualFile(helperVirtualFile);
        assertTrue("imported-source diagnostic is visible on its own file", myFixture.doHighlighting().stream()
                .anyMatch(info -> info.getDescription() != null && info.getDescription().contains(importedDiagnostic.message())));
    }

    public void testMissingImportDiagnosticIsDisplayedWithoutInventingALocalSpan() throws Exception {
        open("missing-import.nagi", "import \"absent.nagi\"\ndef main():\n    print(1)\n");
        var file = myFixture.getFile();
        var service = getProject().getService(NagiAssistService.class);
        waitFor(() -> { service.requestNavigation(file); return service.fresh(file) != null; });
        var diagnostics = service.fresh(file).response().diagnostics();
        assertFalse(diagnostics.isEmpty());
        assertTrue(myFixture.doHighlighting().stream().anyMatch(info -> info.getDescription() != null
                && info.getDescription().contains(diagnostics.getFirst().message())));
    }
}
