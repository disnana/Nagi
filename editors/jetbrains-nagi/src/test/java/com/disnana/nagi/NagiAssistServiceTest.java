package com.disnana.nagi;

import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.util.ui.UIUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BooleanSupplier;

/** Real Platform documents, background snapshots and deterministic cancellation gates. */
public class NagiAssistServiceTest extends BasePlatformTestCase {
    private Path root;
    private NagiAssistService service;
    private boolean originalTrust;
    private String originalCompiler;
    private int originalTimeout;
    @Override protected void setUp() throws Exception {
        super.setUp();
        // Light fixtures reuse a Project whose base directory may have been
        // removed by the previous fixture. Real editor snapshots require an
        // existing project directory, just as the compiler's CLI does.
        Path base = Path.of(getProject().getBasePath());
        Files.createDirectories(base);
        root = Files.createTempDirectory(base, "nagi assist editor ");
        originalTrust = TrustedProjects.isProjectTrusted(getProject());
        originalCompiler = NagiSettings.getInstance().getState().compilerPath;
        originalTimeout = NagiSettings.getInstance().getState().checkTimeoutSeconds;
        TrustedProjects.setProjectTrusted(getProject(), true);
        NagiSettings.update(Path.of(System.getProperty("java.home"), "bin", "java").toString(), 5);
    }
    @Override protected void tearDown() throws Exception {
        try {
            if (service != null) service.dispose();
            NagiSettings.update(originalCompiler, originalTimeout);
            TrustedProjects.setProjectTrusted(getProject(), originalTrust);
            try (var files = Files.walk(root)) {
                for (Path file : files.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
        } finally { super.tearDown(); }
    }
    private PsiFile source(String name, String text) throws Exception {
        Path path = root.resolve(name);
        Files.writeString(path, text);
        var file = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(file);
        myFixture.configureFromExistingVirtualFile(file);
        return myFixture.getFile();
    }
    private NagiAssistProtocol.Response response(String file, String name) {
        return new NagiAssistProtocol.Response("editor-partial", true, false, false, "names", List.of(),
                List.of(new NagiAssistProtocol.CompletionItem(name, "local", "bool", null,
                        new NagiAssistProtocol.Location(file, 2, 5, name.length()), false, "read")),
                List.of(), List.of(file));
    }
    private void waitUntil(BooleanSupplier done) throws Exception {
        long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
        while (!done.getAsBoolean() && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents();
            Thread.sleep(5);
        }
        assertTrue("service state: " + service.state() + ": " + service.statusMessage(), done.getAsBoolean());
    }
    public void testUnsavedSnapshotIsDebouncedAndCurrentResponseIsCached() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    saved = True\n    sa\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            assertFalse(javax.swing.SwingUtilities.isEventDispatchThread());
            assertTrue(guard.getAsBoolean());
            calls.incrementAndGet();
            assertTrue(new String(input, java.nio.charset.StandardCharsets.UTF_8).contains("fresh"));
            return response(file.getVirtualFile().getPath(), "fresh");
        });
        var document = myFixture.getEditor().getDocument();
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.setText("def main():\n    fresh = True\n    fr\n"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        myFixture.getEditor().getCaretModel().moveToOffset(document.getTextLength() - 1);
        for (int i = 0; i < 20; i++) service.requestCompletion(file, myFixture.getEditor());
        waitUntil(() -> service.fresh(file, myFixture.getEditor()) != null);
        assertEquals(1, calls.get());
        assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));
        assertTrue(Files.readString(root.resolve("main.nagi")).contains("saved"));
        service.requestCompletion(file, myFixture.getEditor());
        assertEquals(1, calls.get());
        var cached = service.fresh(file, myFixture.getEditor());
        long started = System.nanoTime();
        for (int query = 0; query < 500; query++) assertSame(cached, service.fresh(file, myFixture.getEditor()));
        System.out.println("NagiAssistCache: 500 current lookups, mean_ms="
                + ((System.nanoTime() - started) / 500_000_000.0) + ", additional_launches=" + (calls.get() - 1));
        assertEquals("cache hits must never launch a process", 1, calls.get());
        myFixture.getEditor().getCaretModel().moveToOffset(0);
        assertNull(service.fresh(file, myFixture.getEditor()));
    }
    public void testEditingCancelsOldRequestAndRejectsLateResponse() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n    va\n");
        AtomicInteger calls = new AtomicInteger();
        CountDownLatch finishOld = new CountDownLatch(1);
        java.util.concurrent.atomic.AtomicReference<NagiAssistProcess.Cancellation> old = new java.util.concurrent.atomic.AtomicReference<>();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            int count = calls.incrementAndGet();
            if (count == 1) {
                old.set(cancellation);
                // Simulate a late transport response even after cancellation.
                while (finishOld.getCount() != 0) {
                    try { finishOld.await(); } catch (InterruptedException ignored) { }
                }
            }
            return response(file.getVirtualFile().getPath(), count == 1 ? "old" : "current");
        });
        service.requestBaseline(file);
        waitUntil(() -> old.get() != null);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> myFixture.getEditor().getDocument().insertString(0, "# changed\n"));
        assertTrue(old.get().isCancelled());
        assertNull(service.fresh(file));
        service.requestBaseline(file);
        finishOld.countDown();
        waitUntil(() -> service.fresh(file) != null);
        assertEquals("current", service.fresh(file).response().completions().getFirst().name());
        assertEquals(2, calls.get());
    }
    public void testUntrustedProjectAndUnsavedManifestNeverLaunchCompiler() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    print(1)\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet(); throw new AssertionError("must not launch");
        });
        TrustedProjects.setProjectTrusted(getProject(), false);
        service.requestBaseline(file);
        assertEquals(NagiAssistService.State.UNTRUSTED, service.state());
        assertEquals(0, calls.get());
        TrustedProjects.setProjectTrusted(getProject(), true);
        var manifest = source("nagi.toml", "entry = \"main.nagi\"\n");
        var manifestDocument = PsiDocumentManager.getInstance(getProject()).getDocument(manifest);
        assertNotNull(manifestDocument);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> manifestDocument.insertString(0, "# unsaved\n"));
        service.requestBaseline(file);
        waitUntil(() -> service.state() == NagiAssistService.State.UNSAVED_MANIFEST);
        assertEquals(0, calls.get());
        assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(manifestDocument));
    }
    public void testDisposeInvalidatesPreviouslyCurrentResponse() throws Exception {
        PsiFile file = source("main.low", "fn main() { let value = true; value; }\n");
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> response(file.getVirtualFile().getPath(), "value"));
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        var result = service.fresh(file);
        service.dispose();
        assertNull(service.fresh(file));
        assertFalse(service.isCurrent(file, result));
        assertEquals(NagiAssistService.State.CLOSED, service.state());
    }
    public void testFailedSnapshotDoesNotRestartForEveryAnnotatorRequest() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    print(1)\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            throw new java.io.IOException("matching compiler unavailable");
        });
        service.requestBaseline(file);
        waitUntil(() -> service.state() == NagiAssistService.State.FAILED);
        for (int i = 0; i < 20; i++) service.requestBaseline(file);
        assertEquals(1, calls.get());
        assertEquals(NagiAssistService.State.FAILED, service.state());
    }

    public void testPreviouslyUnknownClosedDependencyRequiresANewDiskSnapshot() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    print(1)\n");
        Path helper = Files.writeString(root.resolve("helper.low"), "fn helper() { print(1); }\n");
        assertNotNull(LocalFileSystem.getInstance().refreshAndFindFileByNioFile(helper));
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            int call = calls.incrementAndGet();
            if (call == 1) Files.writeString(helper, "fn helper() { print(2); }\n"); // no VFS refresh
            var facts = response(file.getVirtualFile().getPath(), call == 1 ? "obsolete" : "current");
            return new NagiAssistProtocol.Response(facts.semanticStatus(), true, false, false, "names", List.of(),
                    facts.completions(), List.of(), List.of(file.getVirtualFile().getPath(), helper.toString()));
        });
        waitUntil(() -> { service.requestNavigation(file); return service.fresh(file) != null; });
        assertTrue("unknown dependency must be snapshotted before re-analysis", calls.get() >= 2);
        assertEquals("current", service.fresh(file).response().completions().getFirst().name());
    }

    public void testKnownNativeDependencyDiskChangeWithoutVfsRefreshRejectsResponse() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    print(1)\n");
        Path helper = Files.writeString(root.resolve("native.backend"), "fn helper() { print(1); }\n");
        assertNotNull(LocalFileSystem.getInstance().refreshAndFindFileByNioFile(helper));
        var armed = new java.util.concurrent.atomic.AtomicBoolean();
        var entered = new java.util.concurrent.atomic.AtomicBoolean();
        var finish = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            boolean pending = armed.compareAndSet(true, false);
            if (pending) { entered.set(true); finish.await(); }
            var facts = response(file.getVirtualFile().getPath(), pending ? "obsolete" : "current");
            return new NagiAssistProtocol.Response(facts.semanticStatus(), true, false, false, "names", List.of(),
                    facts.completions(), List.of(), List.of(file.getVirtualFile().getPath(), helper.toString()));
        });
        waitUntil(() -> { service.requestNavigation(file); return service.fresh(file) != null; });
        int warmCalls = calls.get();
        armed.set(true);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> myFixture.getEditor().getDocument().insertString(0, "# edit\n"));
        service.requestBaseline(file);
        waitUntil(entered::get);
        try {
            Files.writeString(helper, "fn helper() { print(2); }\n"); // same size, no VFS event
        } finally { finish.countDown(); }
        waitUntil(() -> {
            var current = service.fresh(file);
            if (current != null) assertEquals("a changed native dependency cannot publish obsolete facts", "current", current.response().completions().getFirst().name());
            return current != null;
        });
        assertTrue("changed dependency must be re-analysed", calls.get() >= warmCalls + 2);
    }
}
