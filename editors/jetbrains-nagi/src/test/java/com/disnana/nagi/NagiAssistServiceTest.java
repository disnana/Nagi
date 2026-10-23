package com.disnana.nagi;

import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.codeInsight.lookup.LookupElement;
import com.intellij.openapi.Disposable;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.openapi.vfs.VfsUtil;
import com.intellij.openapi.vfs.VirtualFileManager;
import com.intellij.openapi.vfs.newvfs.BulkFileListener;
import com.intellij.openapi.vfs.newvfs.events.VFileCreateEvent;
import com.intellij.openapi.vfs.newvfs.events.VFileEvent;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.testFramework.ServiceContainerUtil;
import com.intellij.testFramework.PlatformTestUtil;
import com.intellij.util.messages.MessageBus;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BooleanSupplier;
import java.util.function.Supplier;

/** Real Platform documents, background snapshots and deterministic cancellation gates. */
public class NagiAssistServiceTest extends BasePlatformTestCase {
    private Path root;
    private Path externalRoot;
    private NagiAssistService service;
    private com.intellij.openapi.Disposable serviceReplacement;
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
            if (serviceReplacement != null) com.intellij.openapi.util.Disposer.dispose(serviceReplacement);
            else if (service != null) com.intellij.openapi.util.Disposer.dispose(service);
            NagiSettings.update(originalCompiler, originalTimeout);
            TrustedProjects.setProjectTrusted(getProject(), originalTrust);
            try (var files = Files.walk(root)) {
                for (Path file : files.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
            if (externalRoot != null) try (var files = Files.walk(externalRoot)) {
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

    private static NagiAssistProtocol.Response omittedIndex(boolean checked, boolean accepted) {
        return NagiAssistProtocol.decode(("""
                {"format":"nagi-assist-v1","semantic_status":"none",
                 "frontend_checked":%s,"frontend_accepted":%s,"full_compile_checked":false,
                 "recovered":true,"completion":{"kind":"names","access":"read","items":[]},
                 "diagnostics":%s}
                """).formatted(checked, accepted, accepted ? "[]" :
                        "[{\"severity\":\"error\",\"stage\":\"check\",\"message\":\"original rejection\"}]")
                .getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }

    private NagiAssistProtocol.Response retryIndex(String file, String name) {
        var facts = response(file, name);
        return new NagiAssistProtocol.Response("editor-partial", true, false, false, "names",
                omittedIndex(true, false).diagnostics(), facts.completions(), facts.references(), facts.sourceFiles());
    }

    public void testParsedQueryWithoutIndexRetriesOnceWithTheSameUnsavedOverlay() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    saved = True\n");
        var editor = myFixture.getEditor();
        String text = "# 日本語 😀\ndef main():\n    live = True\n";
        WriteCommandAction.runWriteCommandAction(getProject(), () -> editor.getDocument().setText(text));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        editor.getCaretModel().moveToOffset(text.length());
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<com.google.gson.JsonElement> overlays = new AtomicReference<>();
        AtomicReference<NagiAssistProcess.Cancellation> firstCancellation = new AtomicReference<>();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            assertFalse("both requests belong to the worker", ApplicationManager.getApplication().isDispatchThread());
            assertTrue(guard.getAsBoolean());
            var request = com.google.gson.JsonParser.parseString(new String(input, java.nio.charset.StandardCharsets.UTF_8)).getAsJsonObject();
            if (calls.incrementAndGet() == 1) {
                firstCancellation.set(cancellation);
                assertTrue(request.has("query"));
                assertEquals(4, request.getAsJsonObject("query").get("line").getAsInt());
                assertEquals(1, request.getAsJsonObject("query").get("column").getAsInt());
                overlays.set(request.get("files"));
                return omittedIndex(true, false);
            }
            assertFalse("fallback changes only the query", request.has("query"));
            assertSame("the worker owns one cancellation token across both requests", firstCancellation.get(), cancellation);
            assertEquals(overlays.get(), request.get("files"));
            assertEquals(text, request.getAsJsonArray("files").get(0).getAsJsonObject().get("text").getAsString());
            return retryIndex(file.getVirtualFile().getPath(), "never-copy-baseline-candidates");
        });
        service.requestBaseline(file, editor);
        waitUntil(() -> service.fresh(file, editor) != null);
        var result = service.fresh(file, editor).response();
        assertEquals(2, calls.get());
        assertFalse("a type rejection cannot become acceptance", result.frontendAccepted());
        assertTrue(result.recovered());
        assertEquals(omittedIndex(true, false).diagnostics(), result.diagnostics());
        assertEquals("none", result.semanticStatus());
        assertTrue("queryless candidates must not replace original empty completion", result.completions().isEmpty());
        assertEquals(List.of(file.getVirtualFile().getPath()), result.sourceFiles());
        service.requestBaseline(file, editor);
        assertEquals("the combined current result must not loop", 2, calls.get());
        assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(editor.getDocument()));
    }

    public void testPresentIndexKeepsTheSingleRequestFastPath() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            // An explicitly present index may legitimately have no declarations or files.
            return new NagiAssistProtocol.Response("none", true, true, false, "names", List.of(),
                    List.of(), List.of(), List.of());
        });
        service.requestBaseline(file, myFixture.getEditor());
        waitUntil(() -> service.fresh(file) != null);
        assertEquals(1, calls.get());
    }

    public void testOriginalParseFailureDoesNotRequestAQuerylessRetry() throws Exception {
        PsiFile file = source("main.nagi", "def main(:\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            return omittedIndex(false, false);
        });
        service.requestBaseline(file, myFixture.getEditor());
        waitUntil(() -> service.fresh(file) != null);
        assertEquals(1, calls.get());
        assertFalse(service.fresh(file).response().frontendAccepted());
    }

    public void testQuerylessRetryWithoutAnIndexDoesNotLoop() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            return omittedIndex(true, false);
        });
        service.requestBaseline(file, myFixture.getEditor());
        waitUntil(() -> service.fresh(file) != null);
        assertEquals(2, calls.get());
        assertFalse(service.fresh(file).response().frontendAccepted());
        assertTrue(service.fresh(file).response().sourceFiles().isEmpty());
        assertEquals(omittedIndex(true, false).diagnostics(), service.fresh(file).response().diagnostics());
    }

    public void testQuerylessBaselineDoesNotRetryAnOmittedIndex() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            return omittedIndex(true, false);
        });
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        assertEquals(1, calls.get());
    }

    public void testQuerylessTransportFailureDoesNotPublishAnIntermediateResult() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            if (calls.incrementAndGet() == 1) return omittedIndex(true, true);
            throw new java.io.IOException("synthetic queryless failure");
        });
        service.requestBaseline(file, myFixture.getEditor());
        waitUntil(() -> service.state() == NagiAssistService.State.FAILED);
        assertNull(service.fresh(file));
        assertEquals(2, calls.get());
        service.requestBaseline(file, myFixture.getEditor());
        assertEquals(2, calls.get());
    }

    public void testLateQuerylessResultCannotExtendTheOriginalOperationDeadline() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n");
        NagiSettings.update(NagiSettings.getInstance().getState().compilerPath, 1);
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<NagiAssistProcess.Cancellation> token = new AtomicReference<>();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            token.set(cancellation);
            Thread.sleep(800);
            return calls.incrementAndGet() == 1 ? omittedIndex(true, false)
                    : retryIndex(file.getVirtualFile().getPath(), "too-late");
        });
        service.requestBaseline(file, myFixture.getEditor());
        waitUntil(() -> service.state() == NagiAssistService.State.FAILED || service.fresh(file) != null);
        assertEquals("deadline exhaustion is a failure, never READY or an editing cancellation",
                NagiAssistService.State.FAILED, service.state());
        assertNull(service.fresh(file));
        assertEquals(2, calls.get());
        assertFalse("expiry must not masquerade as user cancellation", token.get().isCancelled());
        service.requestBaseline(file, myFixture.getEditor());
        assertEquals("the expired snapshot must not automatically retry", 2, calls.get());
    }


    public void testEditCancelsQuerylessRetryBeforeItsIndexCanPublish() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    old = True\n");
        var editor = myFixture.getEditor();
        CountDownLatch entered = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<NagiAssistProcess.Cancellation> retryCancellation = new AtomicReference<>();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            int call = calls.incrementAndGet();
            if (call == 1) return omittedIndex(true, false);
            if (call == 2) {
                retryCancellation.set(cancellation);
                entered.countDown();
                while (release.getCount() != 0) {
                    try { release.await(); } catch (InterruptedException ignored) { }
                }
                return retryIndex(file.getVirtualFile().getPath(), "obsolete");
            }
            return response(file.getVirtualFile().getPath(), "current");
        });
        service.requestBaseline(file, editor);
        try {
            waitUntil(() -> entered.getCount() == 0);
            assertNull("the first no-index response must never publish early", service.fresh(file));
            WriteCommandAction.runWriteCommandAction(getProject(), () -> editor.getDocument().setText("def main():\n    current = True\n"));
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertTrue(retryCancellation.get().isCancelled());
            service.requestBaseline(file, editor);
        } finally { release.countDown(); }
        waitUntil(() -> service.fresh(file) != null);
        assertEquals("current", service.fresh(file).response().completions().getFirst().name());
        assertEquals(3, calls.get());
    }

    public void testDisagreeingQuerylessVerdictFailsClosedWithoutRepeatedRequests() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    value = True\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                calls.incrementAndGet() == 1 ? omittedIndex(true, false) : omittedIndex(true, true));
        service.requestBaseline(file, myFixture.getEditor());
        waitUntil(() -> service.state() == NagiAssistService.State.FAILED);
        assertNull(service.fresh(file));
        assertEquals(2, calls.get());
        service.requestBaseline(file, myFixture.getEditor());
        assertEquals(2, calls.get());
    }

    public void testRustVerbatimWindowsSourcePathsHaveTheSameIdeIdentity() {
        assertEquals(NagiAssistService.normalizePath("C:\\work\\main.nagi"),
                NagiAssistService.normalizePath("\\\\?\\C:\\work\\main.nagi"));
        assertEquals(NagiAssistService.normalizePath("\\\\server\\share\\main.low"),
                NagiAssistService.normalizePath("\\\\?\\UNC\\server\\share\\main.low"));
    }
    private void waitUntil(BooleanSupplier done) throws Exception {
        long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
        while (!done.getAsBoolean() && System.nanoTime() < deadline) {
            // Let the IDE dispatcher release implicit write intent for background VFS events.
            PlatformTestUtil.dispatchAllInvocationEventsInIdeEventQueue();
            Thread.sleep(5);
        }
        assertTrue("service state: " + service.state() + ": " + service.statusMessage(), done.getAsBoolean());
    }
    private void waitUntil(BooleanSupplier done, Supplier<String> failureMessage) throws Exception {
        long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
        while (!done.getAsBoolean() && System.nanoTime() < deadline) {
            PlatformTestUtil.dispatchAllInvocationEventsInIdeEventQueue();
            Thread.sleep(5);
        }
        boolean complete = done.getAsBoolean();
        assertTrue(complete ? "" : failureMessage.get(), complete);
    }
    private void refreshNioDirectory(Path directory) throws Exception {
        var virtualDirectory = LocalFileSystem.getInstance().findFileByPath(directory.toString());
        assertNotNull("the targeted parent was registered with VFS before the mutation", virtualDirectory);
        VfsUtil.markDirty(false, false, virtualDirectory);
        CountDownLatch refreshed = new CountDownLatch(1);
        virtualDirectory.refresh(true, false, refreshed::countDown);
        waitUntil(() -> refreshed.getCount() == 0, () -> "targeted VFS refresh did not finish for " + directory);
    }
    private static NagiAssistProtocol.Response importSourceResponse(String file, Path directory, String candidateName) {
        var context = new NagiAssistProtocol.ImportContext("import-source",
                new NagiAssistProtocol.Location(file, 1, 8, 2), List.of(directory.toString()), false);
        var item = new NagiAssistProtocol.CompletionItem(candidateName, "module", null, null, null, false, "read");
        return new NagiAssistProtocol.Response("editor-partial", false, false, false, "names", List.of(),
                List.of(item), List.of(), List.of(file), List.of(), List.of(), List.of(), context);
    }
    private static final class CompilerVfsCounters {
        private final String compilerPath;
        private final AtomicInteger exactPathEvents = new AtomicInteger();
        private final AtomicInteger ancestorDirectoryEvents = new AtomicInteger();

        private CompilerVfsCounters(Path compilerPath) {
            this.compilerPath = NagiAssistService.normalizePath(compilerPath.toString());
        }

        private void observe(List<? extends VFileEvent> events) {
            for (VFileEvent event : events) {
                try {
                    String eventPath = NagiAssistService.normalizePath(event.getPath());
                    if (compilerPath.equals(eventPath)) {
                        exactPathEvents.incrementAndGet();
                    } else if (isDirectoryEvent(event) && isStrictAncestor(eventPath, compilerPath)) {
                        ancestorDirectoryEvents.incrementAndGet();
                    }
                } catch (RuntimeException ignored) {
                    // Diagnostic observation must not affect the production event listener.
                }
            }
        }

        private static boolean isDirectoryEvent(VFileEvent event) {
            if (event instanceof VFileCreateEvent createEvent) return createEvent.isDirectory();
            var file = event.getFile();
            return file != null && file.isDirectory();
        }

        private static boolean isStrictAncestor(String possibleAncestor, String path) {
            try {
                Path ancestor = Path.of(possibleAncestor).toAbsolutePath().normalize();
                Path child = Path.of(path).toAbsolutePath().normalize();
                return !ancestor.equals(child) && child.startsWith(ancestor);
            } catch (RuntimeException ignored) {
                return false;
            }
        }

        private String counts(String prefix) {
            return prefix + "Exact=" + exactPathEvents.get()
                    + "," + prefix + "AncestorDirs=" + ancestorDirectoryEvents.get();
        }
    }
    private static void observeCompilerVfs(Disposable parent, MessageBus bus, CompilerVfsCounters counters) {
        var connection = bus.connect(parent);
        connection.subscribe(VirtualFileManager.VFS_CHANGES, new BulkFileListener() {
            @Override public void before(List<? extends VFileEvent> events) { }
            @Override public void after(List<? extends VFileEvent> events) { counters.observe(events); }
        });
    }
    private static java.lang.reflect.Method activeDaemonEditorSetter() throws NoSuchMethodException {
        Class<?> trackerApi = com.intellij.codeInsight.daemon.impl.EditorTracker.class;
        try { return trackerApi.getMethod("setActiveEditorsInTests", List.class); }
        catch (NoSuchMethodException stable251) {
            return trackerApi.getMethod("setActiveEditors", List.class);
        }
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
        PsiFile file = source("main.nagi", "import ut\n");
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(file.getTextLength() - 1);
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet(); throw new AssertionError("must not launch");
        });
        TrustedProjects.setProjectTrusted(getProject(), false);
        service.requestCompletion(file, editor);
        assertEquals(NagiAssistService.State.UNTRUSTED, service.state());
        assertNull("an untrusted import slot has no compiler completion snapshot", service.fresh(file, editor));
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
        Runnable capturedAttach = capturedDocumentRegistration(service, file, myFixture.getEditor().getDocument());
        service.dispose();
        assertNull(service.fresh(file));
        assertFalse(service.isCurrent(file, result));
        assertEquals(NagiAssistService.State.CLOSED, service.state());
        service.dispose();
        // Simulate a queued snapshot captured before direct disposal. The
        // registration boundary itself must refuse it, not just its caller.
        capturedAttach.run();
        assertEquals(0, trackedDocumentCount(service));
        com.intellij.openapi.util.Disposer.dispose(service);
        assertEquals(NagiAssistService.State.CLOSED, service.state());
    }

    public void testParentDisposerReleasesOnlyOwnedDocumentListeners() throws Exception {
        PsiFile file = source("owned.low", "fn main() { let value = true; value; }\n");
        var document = myFixture.getEditor().getDocument();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                response(file.getVirtualFile().getPath(), "value"));
        NagiAssistService other = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                response(file.getVirtualFile().getPath(), "other"));
        // Project services are normally disposed as children of their owner.
        Disposable lifetime = com.intellij.openapi.util.Disposer.newDisposable("owned service lifetime");
        com.intellij.openapi.util.Disposer.register(getTestRootDisposable(), lifetime);
        com.intellij.openapi.util.Disposer.register(lifetime, service);
        com.intellij.openapi.util.Disposer.register(getTestRootDisposable(), other);
        AtomicInteger independentChanges = new AtomicInteger();
        document.addDocumentListener(new com.intellij.openapi.editor.event.DocumentListener() {
            @Override public void documentChanged(com.intellij.openapi.editor.event.DocumentEvent event) {
                independentChanges.incrementAndGet();
            }
        }, getTestRootDisposable());
        try {
            service.requestBaseline(file);
            other.requestBaseline(file);
            waitUntil(() -> service.fresh(file) != null && other.fresh(file) != null);
            assertEquals(1, trackedDocumentCount(service));
            assertEquals(1, trackedDocumentCount(other));
            Disposable subscription = trackedSubscription(service, document);
            AtomicInteger subscriptionCloses = new AtomicInteger();
            com.intellij.openapi.util.Disposer.register(subscription, () -> subscriptionCloses.incrementAndGet());
            com.intellij.openapi.util.Disposer.dispose(lifetime);
            assertEquals(1, subscriptionCloses.get());
            assertEquals(0, trackedDocumentCount(service));
            assertEquals(1, trackedDocumentCount(other));
            WriteCommandAction.runWriteCommandAction(getProject(), () ->
                    document.insertString(document.getTextLength(), "// other listener remains\n"));
            assertEquals(1, independentChanges.get());
            assertEquals(NagiAssistService.State.CLOSED, service.state());
            assertEquals(NagiAssistService.State.STALE, other.state());
            com.intellij.openapi.util.Disposer.dispose(lifetime);
            com.intellij.openapi.util.Disposer.dispose(service);
            service.dispose();
            assertEquals(1, subscriptionCloses.get());
            assertEquals(0, trackedDocumentCount(service));
        } finally {
            com.intellij.openapi.util.Disposer.dispose(lifetime);
            com.intellij.openapi.util.Disposer.dispose(other);
        }
    }

    public void testDocumentCloseReopenKeepsOnlyCurrentListenerUntilDisposal() throws Exception {
        PsiFile file = source("reopen.low", "fn main() { let value = true; value; }\n");
        var virtualFile = file.getVirtualFile();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                response(virtualFile.getPath(), "value"));
        var editors = com.intellij.openapi.fileEditor.FileEditorManager.getInstance(getProject());
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        for (int cycle = 0; cycle < 3; cycle++) {
            assertEquals(1, trackedDocumentCount(service));
            Disposable subscription = trackedSubscription(service, FileDocumentManager.getInstance().getDocument(virtualFile));
            AtomicInteger subscriptionCloses = new AtomicInteger();
            com.intellij.openapi.util.Disposer.register(subscription, () -> subscriptionCloses.incrementAndGet());
            editors.closeFile(virtualFile);
            waitUntil(() -> trackedDocumentCount(service) == 0);
            assertEquals(1, subscriptionCloses.get());
            com.intellij.openapi.util.Disposer.dispose(subscription);
            assertEquals(1, subscriptionCloses.get());
            assertFalse(editors.isFileOpen(virtualFile));
            myFixture.configureFromExistingVirtualFile(virtualFile);
            PsiFile reopened = myFixture.getFile();
            service.requestBaseline(reopened);
            waitUntil(() -> service.fresh(reopened) != null && trackedDocumentCount(service) == 1);
        }
        com.intellij.openapi.util.Disposer.dispose(service);
        assertEquals(0, trackedDocumentCount(service));
        assertEquals(NagiAssistService.State.CLOSED, service.state());
    }

    public void testUnpublishedDocumentSubscriptionIsReleasedOnParentRejectionOrAddFailure() throws Exception {
        PsiFile file = source("rejected.low", "fn main() { let value = true; value; }\n");
        var document = myFixture.getEditor().getDocument();
        AtomicInteger independentChanges = new AtomicInteger();
        document.addDocumentListener(new com.intellij.openapi.editor.event.DocumentListener() {
            @Override public void documentChanged(com.intellij.openapi.editor.event.DocumentEvent event) {
                independentChanges.incrementAndGet();
            }
        }, getTestRootDisposable());
        for (boolean closeParent : new boolean[]{true, false}) {
            service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                    response(file.getVirtualFile().getPath(), "value"));
            NagiAssistService target = service;
            com.intellij.openapi.util.Disposer.register(getTestRootDisposable(), target);
            AtomicReference<Disposable> subscription = new AtomicReference<>();
            AtomicInteger subscriptionCloses = new AtomicInteger();
            // Forward into a real Platform Document and real Disposer. The only
            // injection is immediately after the official add has completed,
            // while the subscription is still private to the registration.
            var intercepted = (com.intellij.openapi.editor.Document) java.lang.reflect.Proxy.newProxyInstance(
                    com.intellij.openapi.editor.Document.class.getClassLoader(),
                    new Class<?>[]{com.intellij.openapi.editor.Document.class}, (proxy, method, args) -> {
                        Object result;
                        try { result = method.invoke(document, args); }
                        catch (java.lang.reflect.InvocationTargetException failure) { throw failure.getTargetException(); }
                        if (method.getName().equals("addDocumentListener") && method.getParameterCount() == 2) {
                            Disposable token = (Disposable) args[1];
                            subscription.set(token);
                            com.intellij.openapi.util.Disposer.register(token, () -> subscriptionCloses.incrementAndGet());
                            if (closeParent) com.intellij.openapi.util.Disposer.dispose(target);
                            else throw new IllegalStateException("owned post-add failure");
                        }
                        return result;
                    });
            Runnable attach = capturedDocumentRegistration(target, file, intercepted);
            try {
                if (closeParent) attach.run();
                else {
                    boolean refused = false;
                    try { attach.run(); }
                    catch (AssertionError failure) {
                        assertTrue(failure.getCause() instanceof java.lang.reflect.InvocationTargetException);
                        assertTrue(failure.getCause().getCause() instanceof IllegalStateException);
                        refused = true;
                    }
                    assertTrue("post-add failure must be propagated", refused);
                }
                assertNotNull(subscription.get());
                assertEquals(1, subscriptionCloses.get());
                assertEquals(0, trackedDocumentCount(target));
                NagiAssistService.State beforeEdit = target.state();
                int otherBefore = independentChanges.get();
                WriteCommandAction.runWriteCommandAction(getProject(), () ->
                        document.insertString(document.getTextLength(), "// only independent listener\n"));
                assertEquals(otherBefore + 1, independentChanges.get());
                assertEquals("released subscription must not invalidate its service", beforeEdit, target.state());
                com.intellij.openapi.util.Disposer.dispose(subscription.get());
                assertEquals(1, subscriptionCloses.get());
            } finally { com.intellij.openapi.util.Disposer.dispose(target); }
        }
    }

    private static Disposable trackedSubscription(NagiAssistService target,
                                                  com.intellij.openapi.editor.Document document) throws Exception {
        var field = NagiAssistService.class.getDeclaredField("documentListeners");
        field.setAccessible(true);
        var listeners = (java.util.Map<?, ?>) field.get(target);
        synchronized (listeners) {
            Object record = listeners.get(document);
            assertNotNull(record);
            var subscription = record.getClass().getDeclaredMethod("subscription");
            subscription.setAccessible(true);
            return (Disposable) subscription.invoke(record);
        }
    }

    private static int trackedDocumentCount(NagiAssistService target) {
        try {
            var field = NagiAssistService.class.getDeclaredField("documentListeners");
            field.setAccessible(true);
            var listeners = (java.util.Map<?, ?>) field.get(target);
            synchronized (listeners) { return listeners.size(); }
        } catch (ReflectiveOperationException failure) { throw new AssertionError(failure); }
    }

    private static Runnable capturedDocumentRegistration(NagiAssistService target, PsiFile file,
                                                          com.intellij.openapi.editor.Document document) throws Exception {
        Class<?> rawFile = Class.forName(NagiAssistService.class.getName() + "$RawFile");
        var constructor = rawFile.getDeclaredConstructor(com.intellij.openapi.vfs.VirtualFile.class,
                com.intellij.openapi.editor.Document.class, String.class, long.class, long.class,
                boolean.class, String.class);
        constructor.setAccessible(true);
        var raw = constructor.newInstance(file.getVirtualFile(), document, file.getVirtualFile().getPath(),
                document.getModificationStamp(), file.getVirtualFile().getModificationStamp(), false, null);
        var register = NagiAssistService.class.getDeclaredMethod("registerDocumentListeners", List.class);
        register.setAccessible(true);
        return () -> {
            try { register.invoke(target, List.of(raw)); }
            catch (ReflectiveOperationException failure) { throw new AssertionError(failure); }
        };
    }

    public void testFailedSnapshotDoesNotRestartForEveryAnnotatorRequest() throws Exception {
        PsiFile file = source("main.nagi", "def main():\n    print(1)\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            throw new java.io.IOException("Nagi assistance ended before responding (startup failure or unsupported protocol). Use published Nagi 0.2.0 from GitHub Releases for assistance. Nagi 0.1.11 supports normal Check/Run only.");
        });
        service.requestBaseline(file);
        waitUntil(() -> service.state() == NagiAssistService.State.FAILED);
        assertNull("failed analysis must not expose cached compiler facts", service.fresh(file));
        assertTrue(service.statusMessage(), service.statusMessage().contains("no cached or saved facts were used."));
        assertTrue(service.statusMessage(), service.statusMessage().contains("Use published Nagi 0.2.0 from GitHub Releases"));
        assertTrue(service.statusMessage(), service.statusMessage().contains("Nagi 0.1.11 supports normal Check/Run only"));
        for (int i = 0; i < 20; i++) service.requestBaseline(file);
        assertEquals(1, calls.get());
        assertEquals(NagiAssistService.State.FAILED, service.state());
    }

    public void testMissingCompilerIsQuietUntilCompilerAppearsAndRejectsOlderResponse() throws Exception {
        PsiFile file = source("main.nagi", "import ut\n");
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(file.getTextLength() - 1);
        Path missingCompiler = root.resolve("tools/" + (com.intellij.openapi.util.SystemInfo.isWindows ? "nagic.exe" : "nagic"));
        Files.createDirectories(missingCompiler.getParent());
        var compilerDirectory = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(missingCompiler.getParent());
        assertNotNull("compiler parent must be known to the IDE VFS before executable creation", compilerDirectory);
        // A first lookup can load an uncached child without a create event.
        // Cache the empty snapshot before the later native creation and refresh.
        ApplicationManager.getApplication().runReadAction(() ->
                assertEquals("compiler directory must be cached while the executable is absent",
                        0, compilerDirectory.getChildren().length));
        NagiSettings.update(missingCompiler.toString(), 5);
        AtomicInteger compilerProbes = new AtomicInteger();
        AtomicInteger calls = new AtomicInteger();
        AtomicInteger notices = new AtomicInteger();
        AtomicReference<Boolean> lastCompilerResolved = new AtomicReference<>();
        AtomicReference<Runnable> retry = new AtomicReference<>();
        AtomicReference<Boolean> analyzedLatestText = new AtomicReference<>(false);
        CountDownLatch oldEntered = new CountDownLatch(1);
        CountDownLatch releaseOld = new CountDownLatch(1);
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            int call = calls.incrementAndGet();
            if (call == 1) {
                oldEntered.countDown();
                while (releaseOld.getCount() != 0) {
                    try { releaseOld.await(); } catch (InterruptedException ignored) { }
                }
                return new NagiAssistProtocol.Response("editor-partial", true, false, false, "names",
                        List.of(new NagiAssistProtocol.Diagnostic("older compiler diagnostic",
                                file.getVirtualFile().getPath(), 2, null)), List.of(), List.of(),
                        List.of(file.getVirtualFile().getPath()));
            }
            analyzedLatestText.set(new String(input, java.nio.charset.StandardCharsets.UTF_8).contains("newer"));
            return new NagiAssistProtocol.Response("editor-partial", true, false, false, "names",
                    List.of(new NagiAssistProtocol.Diagnostic("current compiler diagnostic",
                            file.getVirtualFile().getPath(), 2, null)), List.of(), List.of(),
                    List.of(file.getVirtualFile().getPath()));
        }, (project, retryAction) -> {
            notices.incrementAndGet();
            retry.set(retryAction);
        }, (configured, workspace) -> {
            compilerProbes.incrementAndGet();
            Path resolved = NagiAssistService.resolveCompiler(configured, workspace);
            lastCompilerResolved.set(resolved != null);
            return resolved;
        });

        // VFS invalidation restarts the registered annotator, which resolves the
        // project service. Its automatic request must use this injected runner.
        serviceReplacement = com.intellij.openapi.util.Disposer.newDisposable("Nagi assistance service replacement");
        com.intellij.testFramework.ServiceContainerUtil.replaceService(
                getProject(), NagiAssistService.class, service, serviceReplacement);
        assertSame("annotator and fixture must share the injected service",
                service, getProject().getService(NagiAssistService.class));
        CompilerVfsCounters appVfs = new CompilerVfsCounters(missingCompiler);
        CompilerVfsCounters projectVfs = new CompilerVfsCounters(missingCompiler);
        observeCompilerVfs(serviceReplacement, ApplicationManager.getApplication().getMessageBus(), appVfs);
        observeCompilerVfs(serviceReplacement, getProject().getMessageBus(), projectVfs);

        service.requestCompletion(file, editor);
        waitUntil(() -> service.state() == NagiAssistService.State.COMPILER_UNAVAILABLE && notices.get() == 1);
        assertEquals("initial resolution attempt", 1, compilerProbes.get());
        assertEquals(0, calls.get());
        assertNull("an import slot with no executable has no semantic candidates", service.fresh(file, editor));
        assertNotNull(retry.get());
        assertTrue(service.statusMessage().contains("could not be resolved"));
        assertFalse(NagiAssistExternalAnnotator.shouldRenderFileLevelState(
                NagiAssistService.State.COMPILER_UNAVAILABLE));
        assertTrue(NagiAssistExternalAnnotator.shouldRenderFileLevelState(NagiAssistService.State.FAILED));

        var document = myFixture.getEditor().getDocument();
        for (int edit = 0; edit < 5; edit++) {
            WriteCommandAction.runWriteCommandAction(getProject(), () ->
                    document.insertString(document.getTextLength(), "# typing\n"));
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            service.requestBaseline(file);
            waitUntil(() -> service.state() == NagiAssistService.State.COMPILER_UNAVAILABLE);
            assertEquals("typing must not re-probe the unresolved compiler", 1, compilerProbes.get());
            assertEquals(0, calls.get());
            assertEquals("one quiet notice per compiler setting", 1, notices.get());
            assertNull("missing compiler cannot reuse stale compiler facts", service.fresh(file));
        }

        var daemon = (com.intellij.codeInsight.daemon.impl.DaemonCodeAnalyzerImpl)
                com.intellij.codeInsight.daemon.DaemonCodeAnalyzer.getInstance(getProject());
        var daemonEditors = com.intellij.codeInsight.daemon.impl.EditorTracker.getInstance(getProject());
        var originalDaemonEditors = List.copyOf(daemonEditors.getActiveEditors());
        var setActiveDaemonEditors = activeDaemonEditorSetter();
        var textEditor = com.intellij.openapi.fileEditor.impl.text.TextEditorProvider.getInstance()
                .getTextEditor(myFixture.getEditor());
        try {
            // Headless fixtures have no showing window for the daemon's editor tracker.
            // Use its test-only activation API; normal timer passes still run the annotator.
            setActiveDaemonEditors.invoke(daemonEditors, List.of(myFixture.getEditor()));
            assertEquals("only the fixture editor must be active for automatic daemon passes",
                    1, daemonEditors.getActiveEditors().size());
            assertSame("automatic daemon passes must use the fixture editor",
                    myFixture.getEditor(), daemonEditors.getActiveEditors().getFirst());
            // The light fixture disables timer-driven passes in prepareForTest.
            // Enable only this VFS -> restart -> registered annotator interval.
            daemon.setUpdateByTimerEnabled(true);
            Files.writeString(missingCompiler, "owned test fixture; the injected runner never executes this file\n");
            missingCompiler.toFile().setExecutable(true);
            org.junit.Assume.assumeTrue("filesystem does not expose executable permission", Files.isExecutable(missingCompiler));
            assertNotNull(LocalFileSystem.getInstance().refreshAndFindFileByNioFile(missingCompiler));
            assertTrue("compiler appearance must reach the project VFS listener",
                    projectVfs.exactPathEvents.get() > 0);
            try {
                waitUntil(() -> oldEntered.getCount() == 0, () ->
                        "automatic annotator wait expired; state=" + service.state()
                                + "; status=" + service.statusMessage()
                                + "; compilerProbes=" + compilerProbes.get()
                                + "; calls=" + calls.get()
                                + "; notices=" + notices.get()
                                + "; lastCompilerResolved=" + lastCompilerResolved.get()
                                + "; " + appVfs.counts("appVfs")
                                + "; " + projectVfs.counts("projectVfs")
                                + "; registeredServiceMatches="
                                + (getProject().getService(NagiAssistService.class) == service)
                                + "; nagiHighLanguage=" + (file.getLanguage() == NagiLanguage.HIGH)
                                + "; daemonHighlightingAvailable=" + daemon.isHighlightingAvailable(file)
                                + "; fixtureEditorActive=" + daemonEditors.getActiveEditors().contains(myFixture.getEditor())
                                + "; daemonActiveEditorCount=" + daemonEditors.getActiveEditors().size()
                                + "; initialDaemonActiveEditorCount=" + originalDaemonEditors.size()
                                + "; daemonRunningOrPending=" + daemon.isRunningOrPending()
                                + "; editorLoaded=" + textEditor.isEditorLoaded()
                                + "; backgroundHighlighterPresent=" + (textEditor.getBackgroundHighlighter() != null)
                                + "; documentCommitted=" + PsiDocumentManager.getInstance(getProject()).isCommitted(document)
                                + "; projectInitialized=" + getProject().isInitialized()
                                + "; powerSaveMode=" + com.intellij.ide.PowerSaveMode.isEnabled());
                assertEquals("compiler appearance causes one fresh resolution attempt", 2, compilerProbes.get());
                WriteCommandAction.runWriteCommandAction(getProject(), () -> document.setText(
                        "def main():\n    newer = True\n    print(newer)\n"));
                PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
                service.requestBaseline(file);
            } finally {
                releaseOld.countDown();
            }

            waitUntil(() -> {
                var response = service.fresh(file);
                return response != null && response.response().diagnostics().stream()
                        .anyMatch(diagnostic -> diagnostic.message().equals("current compiler diagnostic"));
            });
            var current = service.fresh(file).response().diagnostics();
            assertEquals(2, calls.get());
            assertEquals("one unavailable probe and one executable verification per analysis",
                    3, compilerProbes.get());
            assertTrue("compiler input must include the latest edit", analyzedLatestText.get());
            assertTrue(current.stream().anyMatch(diagnostic -> diagnostic.message().equals("current compiler diagnostic")));
            assertFalse(current.stream().anyMatch(diagnostic -> diagnostic.message().equals("older compiler diagnostic")));
            assertEquals("one quiet notice is retained after the configured executable appears", 1, notices.get());
        } finally {
            // Stop automatic passes before restoring the active editors and service replacement.
            try { daemon.setUpdateByTimerEnabled(false); }
            finally {
                try { setActiveDaemonEditors.invoke(daemonEditors, originalDaemonEditors); }
                finally { releaseOld.countDown(); }
            }
        }
    }

    public void testCompilerUnavailableIsNotRenderedAsASourceWarning() {
        assertFalse(NagiAssistExternalAnnotator.shouldRenderFileLevelState(
                NagiAssistService.State.COMPILER_UNAVAILABLE));
        assertTrue(NagiAssistExternalAnnotator.shouldRenderFileLevelState(NagiAssistService.State.UNTRUSTED));
        assertTrue(NagiAssistExternalAnnotator.shouldRenderFileLevelState(NagiAssistService.State.FAILED));
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
        service.requestNavigation(file);
        waitUntil(() -> service.fresh(file) != null);
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

    public void testStaleImportCompletionReplyCannotPublishOrOfferOldCandidates() throws Exception {
        PsiFile file = source("main.nagi", "from helper import old\n");
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(file.getText().indexOf("old") + 3);
        CountDownLatch enteredOld = new CountDownLatch(1);
        CountDownLatch releaseOld = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            int call = calls.incrementAndGet();
            if (call == 1) {
                enteredOld.countDown();
                while (releaseOld.getCount() != 0) {
                    try { releaseOld.await(); } catch (InterruptedException ignored) { }
                }
            }
            String text = new String(input, java.nio.charset.StandardCharsets.UTF_8);
            String path = file.getVirtualFile().getPath();
            int nameStart = 19;
            var context = new NagiAssistProtocol.ImportContext("import-names",
                    new NagiAssistProtocol.Location(path, 1, nameStart + 1, text.contains("import newer") ? 5 : 3),
                    List.of(), false);
            var item = new NagiAssistProtocol.CompletionItem(call == 1 ? "obsolete" : "current",
                    "function", null, null, null, false, "read");
            return new NagiAssistProtocol.Response("editor-partial", false, false, false, "names", List.of(),
                    List.of(item), List.of(), List.of(path), List.of(), List.of(), List.of(), context);
        });
        installService();
        service.requestCompletion(file, editor);
        waitUntil(() -> enteredOld.getCount() == 0);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> editor.getDocument().setText("from helper import newer\n"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        editor.getCaretModel().moveToOffset(editor.getDocument().getText().indexOf("newer") + 5);
        assertNull(service.fresh(file, editor));
        service.requestCompletion(file, editor);
        LookupElement[] staleOffer = myFixture.completeBasic();
        assertTrue(staleOffer == null || Arrays.stream(staleOffer)
                .noneMatch(item -> item.getLookupString().equals("obsolete")));
        releaseOld.countDown();
        waitUntil(() -> {
            var current = service.fresh(file, editor);
            return current != null && current.response().completions().stream()
                    .anyMatch(candidate -> candidate.name().equals("current"));
        });
        assertFalse(service.fresh(file, editor).response().completions().stream()
                .anyMatch(candidate -> candidate.name().equals("obsolete")));
        assertTrue(calls.get() >= 2);
    }

    public void testReportedExternalImportDirectoryCreateRenameMoveAndDeleteInvalidateAfterTargetedRefresh() throws Exception {
        externalRoot = Files.createTempDirectory("nagi-assist-external-import-");
        com.intellij.openapi.vfs.newvfs.impl.VfsRootAccess.allowRootAccess(getTestRootDisposable(), externalRoot.toString());
        Path oldParent = Files.createDirectories(externalRoot.resolve("old-parent"));
        Path newParent = Files.createDirectories(externalRoot.resolve("new-parent"));
        assertNotNull(LocalFileSystem.getInstance().refreshAndFindFileByNioFile(newParent));
        Path mainPath = Files.writeString(oldParent.resolve("main.nagi"), "import he\n");
        var mainVirtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(mainPath);
        assertNotNull(mainVirtualFile);
        myFixture.configureFromExistingVirtualFile(mainVirtualFile);
        PsiFile file = myFixture.getFile();
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(file.getTextLength() - 1);
        String path = mainVirtualFile.getPath();
        AtomicReference<NagiAssistProtocol.Response> reply = new AtomicReference<>(
                importSourceResponse(path, oldParent, "helper"));
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            calls.incrementAndGet();
            return reply.get();
        });
        installService();

        service.requestCompletion(file, editor);
        waitUntil(() -> service.fresh(file, editor) != null);
        assertTrue("initial import-directory discovery discards and retries its first compiler response",
                calls.get() >= 2);
        var published = service.fresh(file, editor);
        assertNotNull(published);
        ApplicationManager.getApplication().invokeAndWait(() -> {
            for (int read = 0; read < 500; read++) assertSame(published, service.fresh(file, editor));
        });

        Path created = Files.writeString(oldParent.resolve("created.nagi"), "def created():\n    return 1\n");
        refreshNioDirectory(oldParent);
        assertNull("external creation invalidates after targeted VFS delivery", service.fresh(file, editor));
        service.requestCompletion(file, editor);
        waitUntil(() -> service.fresh(file, editor) != null);

        Path renamed = oldParent.resolve("renamed.nagi");
        Files.move(created, renamed);
        refreshNioDirectory(oldParent);
        assertNull("external rename invalidates after targeted VFS delivery", service.fresh(file, editor));
        service.requestCompletion(file, editor);
        waitUntil(() -> service.fresh(file, editor) != null);

        Path moved = newParent.resolve("moved.nagi");
        Files.move(renamed, moved);
        refreshNioDirectory(oldParent);
        refreshNioDirectory(newParent);
        assertNull("move from an external old parent invalidates after both parents are refreshed",
                service.fresh(file, editor));

        Path newMainPath = Files.writeString(newParent.resolve("main-new.nagi"), "import he\n");
        var newMainVirtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(newMainPath);
        assertNotNull("opening the exact external query path discovers its VFS file", newMainVirtualFile);
        myFixture.configureFromExistingVirtualFile(newMainVirtualFile);
        PsiFile newFile = myFixture.getFile();
        var newEditor = myFixture.getEditor();
        newEditor.getCaretModel().moveToOffset(newFile.getTextLength() - 1);
        String newPath = newMainVirtualFile.getPath();
        int callsBeforeNewParent = calls.get();
        reply.set(importSourceResponse(newPath, newParent, "helper"));
        service.requestCompletion(newFile, newEditor);
        waitUntil(() -> service.fresh(newFile, newEditor) != null);
        assertTrue("newly reported external parent is captured on a retried worker request",
                calls.get() >= callsBeforeNewParent + 2);

        Files.delete(moved);
        refreshNioDirectory(newParent);
        assertNull("external deletion invalidates after targeted VFS delivery", service.fresh(newFile, newEditor));
        service.requestCompletion(newFile, newEditor);
        waitUntil(() -> service.fresh(newFile, newEditor) != null);
        assertTrue("the first directory discovery and the moved-to parent both required retries", calls.get() >= 5);
    }

    public void testUnnotifiedImportDirectoryMutationDuringRunnerDiscardsWorkerResponse() throws Exception {
        externalRoot = Files.createTempDirectory("nagi-assist-import-postcheck-");
        com.intellij.openapi.vfs.newvfs.impl.VfsRootAccess.allowRootAccess(getTestRootDisposable(), externalRoot.toString());
        Path mainPath = Files.writeString(externalRoot.resolve("main.nagi"), "import he\n");
        var mainVirtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(mainPath);
        assertNotNull(mainVirtualFile);
        myFixture.configureFromExistingVirtualFile(mainVirtualFile);
        PsiFile file = myFixture.getFile();
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(file.getTextLength() - 1);
        String path = mainVirtualFile.getPath();
        var discovery = importSourceResponse(path, externalRoot, "discovery");
        var staleDuringRun = importSourceResponse(path, externalRoot, "stale-during-run");
        var afterPostcheck = importSourceResponse(path, externalRoot, "current-after-retry");
        CountDownLatch secondRunnerEntered = new CountDownLatch(1);
        CountDownLatch releaseSecondRunner = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            assertFalse("filesystem-backed analysis must run off the EDT", javax.swing.SwingUtilities.isEventDispatchThread());
            int call = calls.incrementAndGet();
            if (call == 1) return discovery;
            if (call == 2) {
                secondRunnerEntered.countDown();
                releaseSecondRunner.await();
                return staleDuringRun;
            }
            return afterPostcheck;
        });
        installService();

        service.requestCompletion(file, editor);
        try {
            waitUntil(() -> secondRunnerEntered.getCount() == 0);
            var oldDirectoryTime = Files.getLastModifiedTime(externalRoot);
            Files.setLastModifiedTime(externalRoot,
                    java.nio.file.attribute.FileTime.fromMillis(oldDirectoryTime.toMillis() + 10_000));
            assertTrue("directory metadata mutation must be visible to the worker postcheck",
                    !oldDirectoryTime.equals(Files.getLastModifiedTime(externalRoot)));
        } finally {
            releaseSecondRunner.countDown();
        }
        waitUntil(() -> {
            var current = service.fresh(file, editor);
            return current != null && current.response().completions().getFirst().name().equals("current-after-retry");
        });
        assertTrue("the worker's post-run directory check must discard the in-flight facts and retry", calls.get() >= 3);
        assertFalse("the unnotified in-flight compiler result is never published", service.fresh(file, editor)
                .response().completions().stream().anyMatch(item -> item.name().equals("stale-during-run")));
    }

    public void testExpressionCompletionDropsPreviousImportDirectoryInterest() throws Exception {
        externalRoot = Files.createTempDirectory("nagi-assist-import-to-expression-");
        com.intellij.openapi.vfs.newvfs.impl.VfsRootAccess.allowRootAccess(getTestRootDisposable(), externalRoot.toString());
        Path mainPath = Files.writeString(externalRoot.resolve("main.nagi"), "import he\n");
        var mainVirtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(mainPath);
        assertNotNull(mainVirtualFile);
        myFixture.configureFromExistingVirtualFile(mainVirtualFile);
        PsiFile file = myFixture.getFile();
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(file.getTextLength() - 1);
        String path = file.getVirtualFile().getPath();
        var sourceContext = new NagiAssistProtocol.ImportContext("import-source",
                new NagiAssistProtocol.Location(path, 1, 8, 2), List.of(externalRoot.toString()), false);
        var module = new NagiAssistProtocol.CompletionItem("helper", "module", null, null, null, false, "read");
        var importReply = new NagiAssistProtocol.Response("editor-partial", false, false, false, "names", List.of(),
                List.of(module), List.of(), List.of(path), List.of(), List.of(), List.of(), sourceContext);
        AtomicReference<NagiAssistProtocol.Response> reply = new AtomicReference<>(importReply);
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> reply.get());
        installService();

        service.requestCompletion(file, editor);
        waitUntil(() -> service.fresh(file, editor) != null);
        assertEquals("import-source", service.fresh(file, editor).response().importContext().kind());

        WriteCommandAction.runWriteCommandAction(getProject(), () -> editor.getDocument().setText("helper\n"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        editor.getCaretModel().moveToOffset(editor.getDocument().getTextLength() - 1);
        reply.set(response(path, "helper"));
        service.requestCompletion(file, editor);
        waitUntil(() -> {
            var current = service.fresh(file, editor);
            return current != null && current.response().importContext() == null;
        });

        Files.writeString(externalRoot.resolve("late.nagi"), "def late():\n    return 1\n");
        assertNotNull("expression response no longer depends on the previous import directory",
                service.fresh(file, editor));
    }

    public void testImportedTargetOverlayChangesInvalidateCompletionSnapshot() throws Exception {
        Path helperPath = Files.writeString(root.resolve("helper.nagi"), "def build() -> i64:\n    return 1\n");
        var helperVirtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(helperPath);
        assertNotNull(helperVirtualFile);
        com.intellij.openapi.fileEditor.FileEditorManager.getInstance(getProject()).openFile(helperVirtualFile, true);
        var helperDocument = FileDocumentManager.getInstance().getDocument(helperVirtualFile);
        assertNotNull(helperDocument);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> helperDocument.setText(
                "def build() -> i64:\n    return 2\n"));
        PsiFile main = source("main.nagi", "from helper import bu\n");
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(main.getTextLength() - 1);
        String mainPath = main.getVirtualFile().getPath();
        var context = new NagiAssistProtocol.ImportContext("import-names",
                new NagiAssistProtocol.Location(mainPath, 1, 20, 2), List.of(), false);
        var candidate = new NagiAssistProtocol.CompletionItem("build", "function", null, null, null, false, "read");
        AtomicReference<String> captured = new AtomicReference<>();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            captured.set(new String(input, java.nio.charset.StandardCharsets.UTF_8));
            return new NagiAssistProtocol.Response("editor-partial", false, false, false, "names", List.of(),
                    List.of(candidate), List.of(), List.of(mainPath, helperVirtualFile.getPath()),
                    List.of(), List.of(), List.of(), context);
        });
        installService();
        service.requestCompletion(main, editor);
        waitUntil(() -> service.fresh(main, editor) != null);
        assertTrue("unsaved imported target text is included in the compiler overlay",
                captured.get().contains("return 2"));
        WriteCommandAction.runWriteCommandAction(getProject(), () -> helperDocument.setText(
                "def build() -> i64:\n    return 3\n"));
        assertNull("an imported target overlay edit invalidates its completion response", service.fresh(main, editor));
    }

    public void testRegisteredDocumentationUsesHoveredOffsetAndRejectsShadowedOrStaleFacts() throws Exception {
        PsiFile file = source("documentation.nagi", "# 😀\r\nclass Point:\r\n    value: i64\r\n"
                + "def value(x: i64) -> i64:\r\n    return x\r\ndef main():\r\n"
                + "    print(value(1))\r\n    value = 2\r\n    print(value)\r\n    ready()\r\n");
        String path = file.getVirtualFile().getPath();
        var point = new NagiAssistProtocol.Definition("Point", "class", new NagiAssistProtocol.Location(path, 2, 7, 5),
                "class Point\n    value: i64", List.of(), null, false,
                List.of(new NagiAssistProtocol.Parameter("value", "i64")));
        var function = new NagiAssistProtocol.Definition("value", "function", new NagiAssistProtocol.Location(path, 4, 5, 5),
                "def value(x: i64) -> i64", List.of(new NagiAssistProtocol.Parameter("x", "i64")), "i64", false, List.of());
        var standard = new NagiAssistProtocol.Definition("ready", "function", new NagiAssistProtocol.Location("stdlib:async", 1, 1, 5),
                "async def ready() -> unit", List.of(), "unit", true, List.of());
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            assertFalse("documentation analysis never runs on the EDT", javax.swing.SwingUtilities.isEventDispatchThread());
            calls.incrementAndGet();
            return new NagiAssistProtocol.Response("none", true, true, false, "names", List.of(), List.of(), List.of(
                    new NagiAssistProtocol.Reference(new NagiAssistProtocol.Location(path, 7, 11, 5), function.location()),
                    new NagiAssistProtocol.Reference(new NagiAssistProtocol.Location(path, 9, 11, 5), new NagiAssistProtocol.Location(path, 8, 5, 5)),
                    new NagiAssistProtocol.Reference(new NagiAssistProtocol.Location(path, 10, 5, 5), standard.location())),
                    List.of(path), List.of(point, function, standard), List.of());
        });
        installService();
        var provider = com.intellij.codeInsight.documentation.DocumentationManager.getProviderFromElement(file);
        var editor = myFixture.getEditor();
        editor.getCaretModel().moveToOffset(0);
        int offset = file.getText().indexOf("value(1)");
        var hovered = provider.getCustomDocumentationElement(editor, file, file.findElementAt(0), offset + 2);
        assertNotNull("the registered provider must use the hovered token, not the caret", hovered);
        assertEquals("value", hovered.getText());
        assertNull("cold documentation only requests worker warmup", provider.generateDoc(hovered, hovered));
        waitUntil(() -> service.fresh(file) != null);
        String html = provider.generateDoc(hovered, hovered);
        assertNotNull(html);
        assertTrue(html.contains("Returns:</b> <code>i64"));
        assertEquals(1, calls.get());
        var declaration = file.findElementAt(file.getText().indexOf("def value") + 4);
        assertNotNull(provider.generateDoc(declaration, declaration));
        var className = file.findElementAt(file.getText().indexOf("Point"));
        assertTrue(provider.generateDoc(className, className).contains("Fields (use named arguments"));
        var shadow = file.findElementAt(file.getText().lastIndexOf("print(value)") + 6);
        assertNull("same spelling is not a semantic target", provider.generateDoc(shadow, shadow));
        var virtual = file.findElementAt(file.getText().indexOf("ready()"));
        assertTrue("virtual stdlib documentation needs no navigable source PSI", provider.generateDoc(virtual, virtual).contains("Async function"));
        WriteCommandAction.runWriteCommandAction(getProject(), () -> editor.getDocument().insertString(0, "# changed\n"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        var changed = file.findElementAt(file.getText().indexOf("value(1)"));
        assertNull("an edited snapshot cannot display an old return type", provider.generateDoc(changed, changed));
        TrustedProjects.setProjectTrusted(getProject(), false);
        assertNull(provider.generateDoc(changed, changed));
    }

    public void testDiagnosticDescriptionsKeepOriginalTextWhileAllTooltipRangesAreEscaped() throws Exception {
        PsiFile file = source("tooltips.nagi", "def main():\n    print(1)\n\n");
        String path = file.getVirtualFile().getPath();
        String raw = "error: line 2: bad <tag>&lt; (found Id(\"value\"))\n --> " + path + ":2\n 2 |     print(1)\nhint: keep";
        var diagnostics = List.of(
                new NagiAssistProtocol.Diagnostic(raw, path, 2, new NagiAssistProtocol.Location(path, 2, 5, 5)),
                new NagiAssistProtocol.Diagnostic(raw + " range-zero", path, 2, new NagiAssistProtocol.Location(path, 2, 5, 0)),
                new NagiAssistProtocol.Diagnostic(raw + " end", path, 2, new NagiAssistProtocol.Location(path, 2, 13, 0)),
                new NagiAssistProtocol.Diagnostic(raw + " line", path, 2, null),
                new NagiAssistProtocol.Diagnostic("empty <tag>&lt;", path, 3, null),
                new NagiAssistProtocol.Diagnostic("file <tag>&lt;", path, null, null),
                new NagiAssistProtocol.Diagnostic("unknown origin <tag>&lt;", "stdlib:missing", null, null));
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                new NagiAssistProtocol.Response("none", true, false, false, "names", diagnostics, List.of(), List.of(), List.of(path)));
        installService();
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        var highlighted = myFixture.doHighlighting();
        for (var diagnostic : diagnostics) {
            var info = highlighted.stream().filter(value -> diagnostic.message().equals(value.getDescription())).findFirst();
            assertTrue("original diagnostic description must survive every annotation range: " + diagnostic.message(), info.isPresent());
            assertTrue(info.get().getToolTip().contains("&lt;tag&gt;&amp;lt;"));
            assertFalse(info.get().getToolTip().contains("<tag>"));
        }
    }

    private boolean functionRoleAt(int offset, int length) {
        var editor = myFixture.getEditor();
        var expected = editor.getColorsScheme().getAttributes(
                com.intellij.openapi.editor.DefaultLanguageHighlighterColors.FUNCTION_DECLARATION);
        return java.util.Arrays.stream(com.intellij.openapi.editor.impl.DocumentMarkupModel.forDocument(
                editor.getDocument(), getProject(), true).getAllHighlighters()).anyMatch(highlighter ->
                highlighter.getStartOffset() == offset && highlighter.getEndOffset() == offset + length
                        && java.util.Objects.equals(expected, highlighter.getTextAttributes(editor.getColorsScheme())));
    }

    private NagiAssistProtocol.Response roleFacts(PsiFile file, boolean diagnostic) {
        String path = file.getVirtualFile().getPath();
        var location = new NagiAssistProtocol.Location(path, 1, 5, 3);
        return new NagiAssistProtocol.Response("editor-partial", true, false, false, "names",
                diagnostic ? List.of(new NagiAssistProtocol.Diagnostic("old diagnostic", path, 5,
                        new NagiAssistProtocol.Location(path, 5, 15, 1))) : List.of(), List.of(),
                List.of(new NagiAssistProtocol.Reference(new NagiAssistProtocol.Location(path, 5, 11, 3), location)),
                List.of(path), List.of(new NagiAssistProtocol.Definition("add", "function", location,
                        "def add(value: i64) -> i64", List.of(new NagiAssistProtocol.Parameter("value", "i64")),
                        "i64", false, List.of())), List.of());
    }

    public void testPendingAnalysisKeepsOnlyUntouchedRolePresentation() throws Exception {
        PsiFile file = source("steady.nagi", "def add(value: i64) -> i64:\n    return value\n\ndef main():\n    print(add(1))\n");
        CountDownLatch finish = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            if (calls.incrementAndGet() > 1) finish.await();
            return roleFacts(file, true);
        });
        installService();
        try {
            service.requestBaseline(file);
            waitUntil(() -> service.fresh(file) != null);
            myFixture.doHighlighting();
            assertTrue("the real external annotation pass initially paints the declaration", functionRoleAt(4, 3));
            var document = myFixture.getEditor().getDocument();
            int literal = document.getText().lastIndexOf('1');
            WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(literal, literal + 1, "2"));
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertNull(service.fresh(file));
            assertNull("presentation never authorizes old function types", service.documentedDefinition(file, file.findElementAt(4)));
            var pendingHighlights = myFixture.doHighlighting();
            assertTrue("pending analysis must not erase an untouched role color", functionRoleAt(4, 3));
            assertFalse("old diagnostic positions must not survive an edit", pendingHighlights.stream()
                    .anyMatch(info -> "old diagnostic".equals(info.getDescription())));
        } finally { finish.countDown(); }
    }

    public void testRapidEditsKeepDebounceAndCancelOnlyTheActiveAnalysis() throws Exception {
        PsiFile file = source("burst.nagi", "def add(value: i64) -> i64:\n    return value\n\ndef main():\n    print(add(1))\n");
        AtomicInteger starts = new AtomicInteger();
        AtomicReference<NagiAssistProcess.Cancellation> second = new AtomicReference<>();
        CountDownLatch finish = new CountDownLatch(1);
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            if (starts.incrementAndGet() == 2) {
                second.set(cancellation);
                while (finish.getCount() > 0) try { finish.await(); } catch (InterruptedException ignored) { }
            }
            return roleFacts(file, false);
        });
        installService();
        try {
            service.requestBaseline(file);
            waitUntil(() -> service.fresh(file) != null);
            var document = myFixture.getEditor().getDocument();
            service.rolePresentation(file, document, service.fresh(file));
            int literal = document.getText().lastIndexOf('1');
            WriteCommandAction.runWriteCommandAction(getProject(), () -> {
                for (int i = 0; i < 20; i++) {
                    document.replaceString(literal, literal + 1, i % 2 == 0 ? "2" : "1");
                    service.requestBaseline(file);
                }
            });
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertEquals("a typing burst must not start a process per edit", 1, starts.get());
            assertFalse(service.rolePresentation(file, document, null).isEmpty());
            waitUntil(() -> second.get() != null);
            WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(literal, literal + 1, "3"));
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertTrue("editing cancels the one active worker", second.get().isCancelled());
            assertNull(service.fresh(file));
            service.requestBaseline(file);
            finish.countDown();
            waitUntil(() -> service.fresh(file) != null);
            assertEquals("initial, one coalesced burst and one replacement analysis", 3, starts.get());
        } finally { finish.countDown(); }
    }

    public void testFreshFailureAndSettingsChangesDiscardPresentation() throws Exception {
        PsiFile file = source("failure.nagi", "def add(value: i64) -> i64:\n    return value\n\ndef main():\n    print(add(1))\n");
        AtomicInteger calls = new AtomicInteger();
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            if (calls.incrementAndGet() == 2) throw new java.io.IOException("controlled analysis failure");
            return roleFacts(file, false);
        });
        installService();
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        var document = myFixture.getEditor().getDocument();
        assertFalse(service.rolePresentation(file, document, service.fresh(file)).isEmpty());
        int literal = document.getText().lastIndexOf('1');
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(literal, literal + 1, "2"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        assertFalse(service.rolePresentation(file, document, null).isEmpty());
        service.requestBaseline(file);
        waitUntil(() -> service.state() == NagiAssistService.State.FAILED);
        assertTrue("transport failure removes every retained role", service.rolePresentation(file, document, null).isEmpty());
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(literal, literal + 1, "3"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        assertFalse(service.rolePresentation(file, document, service.fresh(file)).isEmpty());
        NagiSettings.update(NagiSettings.getInstance().getState().compilerPath, 6);
        assertTrue("configuration changes are hard invalidations", service.rolePresentation(file, document, null).isEmpty());
        assertNull(service.fresh(file));
    }

    private NagiSemanticPresentation presentationStore() throws Exception {
        var field = NagiAssistService.class.getDeclaredField("presentation");
        field.setAccessible(true);
        return (NagiSemanticPresentation) field.get(service);
    }

    private void paintCurrentRoles(PsiFile file) throws Exception {
        service.requestBaseline(file);
        waitUntil(() -> service.fresh(file) != null);
        var document = PsiDocumentManager.getInstance(getProject()).getDocument(file);
        assertFalse(service.rolePresentation(file, document, service.fresh(file)).isEmpty());
        assertTrue(presentationStore().retainedRoleCount() > 0);
    }

    public void testTrustFileCloseAndDisposalReleaseAllRoleMarkers() throws Exception {
        PsiFile file = source("lifecycle.nagi", "def add(value: i64) -> i64:\n    return value\n\ndef main():\n    print(add(1))\n");
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> roleFacts(file, false));
        installService();
        paintCurrentRoles(file);
        TrustedProjects.setProjectTrusted(getProject(), false);
        assertTrue(service.rolePresentation(file, myFixture.getEditor().getDocument(), null).isEmpty());
        assertEquals(0, presentationStore().retainedRoleCount());
        TrustedProjects.setProjectTrusted(getProject(), true);
        paintCurrentRoles(file);
        com.intellij.openapi.fileEditor.FileEditorManager.getInstance(getProject()).closeFile(file.getVirtualFile());
        assertEquals("closing a source releases all owned markers", 0, presentationStore().retainedRoleCount());
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        paintCurrentRoles(myFixture.getFile());
        service.dispose();
        assertEquals(0, presentationStore().retainedRoleCount());
        assertNull(service.fresh(myFixture.getFile()));
    }

    private void installService() {
        serviceReplacement = com.intellij.openapi.util.Disposer.newDisposable("Nagi import assistance service replacement");
        ServiceContainerUtil.replaceService(getProject(), NagiAssistService.class, service, serviceReplacement);
    }
}
