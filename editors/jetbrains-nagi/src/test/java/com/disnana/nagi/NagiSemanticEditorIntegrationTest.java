package com.disnana.nagi;

import com.intellij.codeInsight.hint.ParameterInfoControllerBase;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.hint.ParameterInfoControllerData;
import com.intellij.codeInsight.hint.ShowParameterInfoHandler;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors;
import com.intellij.openapi.util.Disposer;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.util.ui.UIUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;

/** Compiler facts rendered through the registered parameter-info controller. */
public final class NagiSemanticEditorIntegrationTest extends BasePlatformTestCase {
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
        root = Files.createTempDirectory(base, "nagi semantic integration ");
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
                    for (Path file : files.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
                }
            }
        } finally { super.tearDown(); }
    }

    private void waitFor(BooleanSupplier condition) throws Exception {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
        while (!condition.getAsBoolean() && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents();
            Thread.sleep(5);
        }
        var service = getProject().getService(NagiAssistService.class);
        assertTrue(service.state() + ": " + service.statusMessage(), condition.getAsBoolean());
    }

    private static ParameterInfoControllerData data(ParameterInfoControllerBase controller) throws Exception {
        // Test-only observation of the actual platform controller; production
        // uses only the public ParameterInfoHandler extension point.
        var field = ParameterInfoControllerBase.class.getDeclaredField("myParameterInfoControllerData");
        field.setAccessible(true);
        return (ParameterInfoControllerData) field.get(controller);
    }

    public void testColdParameterInfoShowsAfterTheCurrentCompilerResponse() throws Exception {
        boolean original = CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO;
        CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO = false;
        try { checkColdExplicitHint(); }
        finally { CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO = original; }
    }

    private void checkColdExplicitHint() throws Exception {
        String text = "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    print(add(1, 2))\n";
        Path path = root.resolve("cold.nagi");
        Files.writeString(path, text);
        var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(virtualFile);
        myFixture.configureFromExistingVirtualFile(virtualFile);
        var file = myFixture.getFile();
        var editor = myFixture.getEditor();
        int opening = text.lastIndexOf("add(") + 3;
        editor.getCaretModel().moveToOffset(opening + 1);
        var service = getProject().getService(NagiAssistService.class);
        assertNull(service.fresh(file));
        new ShowParameterInfoHandler().invoke(getProject(), editor, file);
        waitFor(() -> service.fresh(file) != null);
        assertNotNull(service.functionDefinition(file, file.findElementAt(opening - 3)));
        waitFor(() -> ParameterInfoControllerBase.findControllerAtOffset(editor, opening) != null);
        var controller = ParameterInfoControllerBase.findControllerAtOffset(editor, opening);
        assertEquals("add", ((NagiAssistProtocol.Definition) controller.getObjects()[0]).name());
        Disposer.dispose(controller);
    }

    public void testLateParameterHintRespectsCaretAndDisabledAutomaticHints() throws Exception {
        boolean original = CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO;
        try {
            for (boolean moveCaret : new boolean[]{true, false}) {
                CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO = true;
                String text = "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    print(add(1, 2))\n";
                Path path = root.resolve(moveCaret ? "moved.nagi" : "disabled.nagi");
                Files.writeString(path, text);
                var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
                assertNotNull(virtualFile);
                myFixture.configureFromExistingVirtualFile(virtualFile);
                var file = myFixture.getFile();
                var editor = myFixture.getEditor();
                int opening = text.lastIndexOf("add(") + 3;
                editor.getCaretModel().moveToOffset(opening + 1);
                var service = getProject().getService(NagiAssistService.class);
                service.requestParameterInfo(file, editor);
                if (moveCaret) editor.getCaretModel().moveToOffset(0);
                else CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO = false;
                waitFor(() -> service.fresh(file) != null);
                UIUtil.dispatchAllInvocationEvents();
                assertNull("late hints must respect current caret and settings",
                        ParameterInfoControllerBase.findControllerAtOffset(editor, opening));
            }
        } finally { CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO = original; }
    }

    public void testCompleteHighLowCallsUseCompilerTypesWithoutACompletionContext() throws Exception {
        for (boolean low : new boolean[]{false, true}) {
            String text = low
                    ? "record Point { value: i64; }\nfn add(left: i64, right: i64) -> i64 { return left + right; }\nfn read(point: Point) -> i64 { return point.value; }\nfn main() { let point = Point(value=1); print(add(read(point), 2)); }\n"
                    : "class Point:\n    value: i64\ndef add(left: i64, right: i64) -> i64:\n    return left + right\ndef read(point: Point) -> i64:\n    return point.value\ndef main():\n    point = Point(value=1)\n    print(add(read(point), 2))\n";
            Path path = root.resolve(low ? "semantic.low" : "semantic.nagi");
            Files.writeString(path, text);
            var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
            assertNotNull(virtualFile);
            myFixture.configureFromExistingVirtualFile(virtualFile);
            var file = myFixture.getFile();
            var editor = myFixture.getEditor();
            int calleeOffset = text.lastIndexOf("add(");
            int opening = calleeOffset + 3;
            int first = text.indexOf("read(point)", opening);
            int second = text.indexOf(", 2", first) + 2;
            editor.getCaretModel().moveToOffset(second);
            var service = getProject().getService(NagiAssistService.class);
            service.requestBaseline(file, editor);
            waitFor(() -> service.fresh(file) != null);
            var response = service.fresh(file);
            assertTrue(response.response().frontendAccepted());
            assertEquals("none", response.response().semanticStatus());
            var definition = service.functionDefinition(file, file.findElementAt(calleeOffset));
            assertNotNull("accepted typed symbols must not depend on completion context", definition);
            assertEquals("add", definition.name());
            assertEquals("right", definition.parameters().get(1).name());
            assertEquals("i64", definition.parameters().get(1).type());
            var colors = service.semanticColors(file, response);
            assertTrue(colors.stream().anyMatch(c -> c.attributes() == DefaultLanguageHighlighterColors.PARAMETER));
            assertTrue(colors.stream().anyMatch(c -> c.attributes() == DefaultLanguageHighlighterColors.INSTANCE_FIELD));
            assertTrue(colors.stream().anyMatch(c -> c.attributes() == DefaultLanguageHighlighterColors.LOCAL_VARIABLE));

            var call = NagiParameterInfoHandler.callAt(file, editor, editor.getCaretModel().getOffset());
            assertNotNull("bounded parser finds this call", call);
            assertEquals(opening, call.opening().getTextOffset());
            assertTrue("parameter-info extension must be registered", java.util.Arrays.stream(
                    ShowParameterInfoHandler.getHandlers(getProject(), file.getLanguage()))
                    .anyMatch(handler -> handler instanceof NagiParameterInfoHandler));
            assertTrue("platform showHint requires a visible editor component", editor.getComponent().isVisible());
            new ShowParameterInfoHandler().invoke(getProject(), editor, file);
            waitFor(() -> ParameterInfoControllerBase.findControllerAtOffset(editor, opening) != null);
            var controller = ParameterInfoControllerBase.findControllerAtOffset(editor, opening);
            assertEquals(definition, controller.getObjects()[0]);
            var state = data(controller);
            waitFor(() -> state.getCurrentParameterIndex() == 1);
            editor.getCaretModel().moveToOffset(first);
            controller.updateComponent();
            waitFor(() -> state.getCurrentParameterIndex() == 0);
            Disposer.dispose(controller);

            WriteCommandAction.runWriteCommandAction(getProject(), () -> editor.getDocument().insertString(0, "# edited\n"));
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertNull("old coordinates cannot survive an edit", service.functionDefinition(file, file.findElementAt(calleeOffset + 9)));
            assertTrue(service.semanticColors(file, response).isEmpty());
        }
    }
}
