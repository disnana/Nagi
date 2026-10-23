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
            assertEquals("i64", definition.returnType());
            var documentation = com.intellij.codeInsight.documentation.DocumentationManager.getProviderFromElement(file);
            // Exercise registered documentation dispatch at an offset different
            // from the parameter-info caret, using real compiler JSON facts.
            var hovered = documentation.getCustomDocumentationElement(editor, file, file.findElementAt(second), calleeOffset + 1);
            assertNotNull(hovered);
            String functionDoc = documentation.generateDoc(hovered, hovered);
            assertNotNull(functionDoc);
            assertTrue(functionDoc.contains("left: i64"));
            assertTrue(functionDoc.contains("Returns:</b> <code>i64"));
            int pointUse = text.indexOf("Point(value=");
            var pointElement = file.findElementAt(pointUse);
            var pointDefinition = service.documentedDefinition(file, pointElement);
            assertNotNull(pointDefinition);
            assertEquals(java.util.List.of(new NagiAssistProtocol.Parameter("value", "i64")), pointDefinition.fields());
            String classDoc = documentation.generateDoc(pointElement, pointElement);
            assertNotNull(classDoc);
            assertTrue(classDoc.contains("Fields (use named arguments"));
            assertTrue(classDoc.contains("value: i64"));
            assertFalse(classDoc.contains("Point()"));
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

    private String modernDocumentation(com.intellij.psi.PsiFile file, int offset) throws Exception {
        var editor = myFixture.getEditor();
        var indicator = new com.intellij.openapi.progress.EmptyProgressIndicator();
        var finished = new java.util.concurrent.CountDownLatch(1);
        var future = com.intellij.openapi.application.ApplicationManager.getApplication().executeOnPooledThread(
                (java.util.concurrent.Callable<String>) () -> {
                    try { return com.intellij.openapi.progress.ProgressManager.getInstance().runProcess(
                        () -> com.intellij.openapi.progress.CoroutinesKt.runBlockingCancellable((scope, continuation) ->
                                com.intellij.openapi.progress.CoroutinesKt.blockingContext(() ->
                                        com.intellij.openapi.application.ReadAction.compute(() -> {
                                            var targets = com.intellij.lang.documentation.ide.IdeDocumentationTargetProvider.getInstance(getProject())
                                                    .documentationTargets(editor, file, offset);
                                            assertFalse("the platform must create a hover target at the requested offset", targets.isEmpty());
                                            var result = targets.getFirst().computeDocumentation();
                                            return result instanceof com.intellij.platform.backend.documentation.DocumentationData data ? data.getHtml() : null;
                                        }), continuation)), indicator); }
                    finally { finished.countDown(); }
                });
        try {
            waitFor(future::isDone);
            return future.get(1, TimeUnit.SECONDS);
        } finally {
            if (!future.isDone()) {
                indicator.cancel();
                future.cancel(true);
            }
            waitFor(() -> finished.getCount() == 0);
        }
    }

    public void testModernPlatformDocumentationDispatchUsesActualHoverOffset() throws Exception {
        String text = "def add(left: i64, right: i64) -> i64:\n    return left + right\ndef main():\n    print(add(1, 2))\n";
        Path path = root.resolve("modern-hover.nagi");
        Files.writeString(path, text);
        var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(virtualFile);
        myFixture.configureFromExistingVirtualFile(virtualFile);
        var file = myFixture.getFile();
        myFixture.getEditor().getCaretModel().moveToOffset(0);
        var service = getProject().getService(NagiAssistService.class);
        service.requestBaseline(file);
        waitFor(() -> service.fresh(file) != null);
        String html = modernDocumentation(file, text.lastIndexOf("add(") + 1);
        assertNotNull("mouse-hover platform target must expose compiler documentation while the caret is elsewhere", html);
        assertTrue(html.contains("left: i64"));
        assertTrue(html.contains("Returns:</b> <code>i64"));
        assertFalse("frontend assistance must not create a native Cargo project", Files.exists(root.resolve("target")));
    }

    private String platformDocumentation(com.intellij.psi.PsiFile file, int offset) throws Exception {
        var editor = myFixture.getEditor();
        var indicator = new com.intellij.openapi.progress.EmptyProgressIndicator();
        var finished = new java.util.concurrent.CountDownLatch(1);
        var future = com.intellij.openapi.application.ApplicationManager.getApplication().executeOnPooledThread(
                (java.util.concurrent.Callable<String>) () -> {
                    try {
                        var pointer = NagiPlatform.read(() -> {
                            var targets = com.intellij.lang.documentation.ide.IdeDocumentationTargetProvider.getInstance(getProject())
                                    .documentationTargets(editor, file, offset);
                            assertFalse("actual platform dispatch must create a target", targets.isEmpty());
                            assertEquals("plugin.xml must dispatch to the registered async provider, not the legacy fallback",
                                    NagiDocumentationTargetProvider.class, targets.getFirst().getClass().getEnclosingClass());
                            return targets.getFirst().createPointer();
                        });
                        // Test-only SDK consumer: resolves both synchronous and async results,
                        // including the platform's own read-action/indicator transition.
                        var data = com.intellij.openapi.progress.ProgressManager.getInstance().runProcess(
                                () -> com.intellij.platform.backend.documentation.impl.ImplKt.computeDocumentationBlocking(pointer), indicator);
                        return data == null ? null : data.getHtml();
                    } finally { finished.countDown(); }
                });
        try {
            waitFor(future::isDone);
            return future.get(1, TimeUnit.SECONDS);
        } finally {
            if (!future.isDone()) { indicator.cancel(); future.cancel(true); }
            waitFor(() -> finished.getCount() == 0);
        }
    }

    public void testColdPlatformConsumerCompletesSameHoverWithRealCompiler() throws Exception {
        for (boolean low : new boolean[]{false, true}) {
            String text = low
                    ? "# 😀\r\nrecord Point { value: i64; }\r\nfn add(value: i64) -> i64 { return value; }\r\nfn main() { let point = Point(value=7); print(add(point.value)); }\r\n"
                    : "# 😀\r\nclass Point:\r\n    value: i64\r\ndef add(value: i64) -> i64:\r\n    return value\r\ndef main():\r\n    point = Point(value=7)\r\n    print(add(point.value))\r\n";
            Path path = root.resolve(low ? "cold-hover.low" : "cold-hover.nagi");
            Files.writeString(path, text);
            assertTrue("disk fixture starts with CRLF", Files.readString(path).contains("\r\n"));
            var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
            assertNotNull(virtualFile);
            myFixture.configureFromExistingVirtualFile(virtualFile);
            var file = myFixture.getFile();
            String current = file.getText();
            assertEquals(low ? NagiLanguage.LOW : NagiLanguage.HIGH, file.getLanguage());
            myFixture.getEditor().getCaretModel().moveToOffset(0);
            var service = getProject().getService(NagiAssistService.class);
            assertNull("this hover starts without compiler facts", service.fresh(file));
            String html = platformDocumentation(file, current.lastIndexOf("add(") + 1);
            var observed = service.fresh(file);
            assertNotNull("cold consumer must finish with a current compiler snapshot", observed);
            assertTrue(observed.response().diagnostics().toString(), observed.response().frontendAccepted());
            assertNotNull("the same cold platform request must finish with documentation (low=" + low + ")", html);
            assertTrue(html, html.contains("value: i64"));
            assertTrue(html.contains("Returns:</b> <code>i64"));
            assertTrue(platformDocumentation(file, current.indexOf("Point(value=") + 1).contains("Fields (use named arguments"));
            String local = platformDocumentation(file, current.lastIndexOf("point.value") + 1);
            assertNotNull("exact compiler local facts are useful without a callable definition", local);
            assertTrue(local.contains("point: Point"));
            assertFalse(local.contains("Parameters"));
            assertNull("builtin print has no authoritative signature in this protocol",
                    platformDocumentation(file, current.lastIndexOf("print(") + 1));
            assertFalse(Files.exists(root.resolve("target")));
        }
    }

    public void testColdNamedImportHoverUsesCompilerAliasAndShadowFacts() throws Exception {
        Files.writeString(root.resolve("helper.nagi"), "def make() -> i64:\n    return 1\n");
        assertNotNull(LocalFileSystem.getInstance().refreshAndFindFileByNioFile(root.resolve("helper.nagi")));
        String text = "# 😀\r\nfrom \"helper.nagi\" import make as alias\r\ndef main():\r\n    print(alias())\r\n    alias = 2\r\n    print(alias)\r\n";
        Path path = root.resolve("alias-hover.nagi");
        Files.writeString(path, text);
        var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(virtualFile);
        myFixture.configureFromExistingVirtualFile(virtualFile);
        var file = myFixture.getFile();
        String current = file.getText();
        assertNull(getProject().getService(NagiAssistService.class).fresh(file));
        String html = platformDocumentation(file, current.indexOf("alias()") + 2);
        var observed = getProject().getService(NagiAssistService.class).fresh(file);
        assertNotNull("import consumer must finish with a current compiler snapshot", observed);
        assertTrue(observed.response().diagnostics().toString(), observed.response().frontendAccepted());
        assertNotNull(html);
        assertTrue(html.contains("def make() -&gt; i64"));
        String local = platformDocumentation(file, current.lastIndexOf("alias)") + 1);
        assertNotNull(local);
        assertTrue(local.contains("alias: i64"));
        assertFalse("shadowed local must not inherit the import signature", local.contains("Returns:"));
    }

    public void testHighLowAsyncDocumentationUsesCompilerReturnAndAsyncFacts() throws Exception {
        for (boolean low : new boolean[]{false, true}) {
            String text = low
                    ? "async fn fetch(value: i64) -> i64 { return value; }\nasync fn main() { print(await fetch(7)); }\n"
                    : "async def fetch(value: i64) -> i64:\n    return value\nasync def main():\n    print(await fetch(7))\n";
            Path path = root.resolve(low ? "async.low" : "async.nagi");
            Files.writeString(path, text);
            var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
            assertNotNull(virtualFile);
            myFixture.configureFromExistingVirtualFile(virtualFile);
            var file = myFixture.getFile();
            var service = getProject().getService(NagiAssistService.class);
            service.requestBaseline(file);
            waitFor(() -> service.fresh(file) != null);
            assertTrue(service.fresh(file).response().frontendAccepted());
            var use = file.findElementAt(text.lastIndexOf("fetch(7)"));
            var definition = service.documentedDefinition(file, use);
            assertNotNull(definition);
            assertTrue(definition.asynchronous());
            assertEquals("i64", definition.returnType());
            var documentation = com.intellij.codeInsight.documentation.DocumentationManager.getProviderFromElement(file);
            String html = documentation.generateDoc(use, use);
            assertNotNull(html);
            assertTrue(html.contains("Async function"));
            assertTrue(html.contains("Returns:</b> <code>i64"));
            assertTrue(html.contains(low ? "async fn fetch" : "async def fetch"));
        }
    }
}
