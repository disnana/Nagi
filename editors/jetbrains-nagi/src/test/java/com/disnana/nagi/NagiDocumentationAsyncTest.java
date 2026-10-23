package com.disnana.nagi;

import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.fileEditor.FileEditorManager;
import com.intellij.openapi.progress.EmptyProgressIndicator;
import com.intellij.openapi.progress.ProgressManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.platform.backend.documentation.DocumentationData;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import com.intellij.testFramework.ServiceContainerUtil;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.util.ui.UIUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BooleanSupplier;

/** Barrier-controlled real platform consumers; the compiler worker is shared and bounded. */
public final class NagiDocumentationAsyncTest extends BasePlatformTestCase {
    private Path root;
    private PsiFile file;
    private Document document;
    private NagiAssistService service;
    private com.intellij.openapi.Disposable replacement;
    private String originalCompiler;
    private int originalTimeout;
    private boolean originalTrust;
    private final CountDownLatch release = new CountDownLatch(1);
    private final AtomicInteger starts = new AtomicInteger();
    private final AtomicReference<NagiAssistProcess.Cancellation> cancellation = new AtomicReference<>();
    private final List<Hover> hovers = new ArrayList<>();
    private record Hover(Future<DocumentationData> future, EmptyProgressIndicator indicator, CountDownLatch finished,
                         AtomicReference<kotlinx.coroutines.Job> job) {}

    @Override protected void setUp() throws Exception {
        super.setUp();
        Path base = Path.of(getProject().getBasePath());
        Files.createDirectories(base);
        root = Files.createTempDirectory(base, "nagi async documentation ");
        originalCompiler = NagiSettings.getInstance().getState().compilerPath;
        originalTimeout = NagiSettings.getInstance().getState().checkTimeoutSeconds;
        originalTrust = TrustedProjects.isProjectTrusted(getProject());
        NagiSettings.update(Path.of(System.getProperty("java.home"), "bin", "java").toString(), 5);
        TrustedProjects.setProjectTrusted(getProject(), true);
        file = open("main.nagi", "def add(value: i64) -> i64:\n    return value\ndef main():\n    print(add(1))\n");
        document = myFixture.getEditor().getDocument();
    }

    @Override protected void tearDown() throws Exception {
        try {
            release.countDown();
            for (Hover hover : hovers) {
                if (!hover.future().isDone()) {
                    if (hover.job().get() != null) hover.job().get().cancel(new java.util.concurrent.CancellationException("test cleanup"));
                    hover.indicator().cancel(); hover.future().cancel(true);
                }
                waitUntil(() -> hover.finished().getCount() == 0);
            }
            if (replacement != null) com.intellij.openapi.util.Disposer.dispose(replacement);
            NagiSettings.update(originalCompiler, originalTimeout);
            TrustedProjects.setProjectTrusted(getProject(), originalTrust);
            try (var paths = Files.walk(root)) {
                for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        } finally { super.tearDown(); }
    }

    private PsiFile open(String name, String text) throws Exception {
        Path path = root.resolve(name);
        Files.writeString(path, text);
        var virtual = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(virtual);
        myFixture.configureFromExistingVirtualFile(virtual);
        return myFixture.getFile();
    }

    private NagiAssistProtocol.Response facts() {
        String path = file.getVirtualFile().getPath();
        var definition = new NagiAssistProtocol.Definition("add", "function", new NagiAssistProtocol.Location(path, 1, 5, 3),
                "def add(value: i64) -> i64", List.of(new NagiAssistProtocol.Parameter("value", "i64")), "i64", false, List.of());
        return new NagiAssistProtocol.Response("none", true, true, false, "names", List.of(), List.of(),
                List.of(new NagiAssistProtocol.Reference(new NagiAssistProtocol.Location(path, 4, 11, 3), definition.location())),
                List.of(path), List.of(definition), List.of());
    }

    private void install(NagiAssistProcess.Runner runner) {
        service = new NagiAssistService(getProject(), runner);
        replacement = com.intellij.openapi.util.Disposer.newDisposable("async documentation replacement");
        ServiceContainerUtil.replaceService(getProject(), NagiAssistService.class, service, replacement);
    }

    private void blocked() {
        install((plan, input, timeout, cancelled, guard) -> {
            starts.incrementAndGet(); cancellation.set(cancelled); release.await(); return facts();
        });
    }

    private void waitUntil(BooleanSupplier predicate) throws Exception {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
        while (!predicate.getAsBoolean() && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents(); Thread.sleep(5);
        }
        assertTrue("async documentation did not reach its expected boundary", predicate.getAsBoolean());
    }

    private CompletableFuture<Boolean> ticket() {
        return NagiPlatform.read(() -> service.documentationTicket(file, document,
                document.getModificationStamp(), service.documentationEpoch()));
    }

    private Object serviceLock() throws Exception {
        var field = NagiAssistService.class.getDeclaredField("lock"); field.setAccessible(true); return field.get(service);
    }

    private int count() throws Exception {
        var field = NagiAssistService.class.getDeclaredField("documentationTickets"); field.setAccessible(true);
        synchronized (serviceLock()) { return ((List<?>) field.get(service)).size(); }
    }

    private Hover hover() {
        var editor = myFixture.getEditor();
        var indicator = new EmptyProgressIndicator();
        var finished = new CountDownLatch(1);
        var job = new AtomicReference<kotlinx.coroutines.Job>();
        var future = com.intellij.util.concurrency.AppExecutorUtil.getAppExecutorService().submit((java.util.concurrent.Callable<DocumentationData>) () -> {
            try {
                var pointer = NagiPlatform.read(() -> {
                    var targets = com.intellij.lang.documentation.ide.IdeDocumentationTargetProvider.getInstance(getProject())
                            .documentationTargets(editor, file, file.getText().lastIndexOf("add(") + 1);
                    assertFalse(targets.isEmpty()); return targets.getFirst().createPointer();
                });
                return ProgressManager.getInstance().runProcess(
                        () -> com.intellij.openapi.progress.CoroutinesKt.<DocumentationData>runBlockingCancellable((scope, continuation) -> {
                            var deferred = com.intellij.platform.backend.documentation.impl.ImplKt.computeDocumentationAsync(scope, pointer);
                            job.set(deferred);
                            return deferred.await(continuation);
                        }), indicator);
            } finally { finished.countDown(); }
        });
        var hover = new Hover(future, indicator, finished, job); hovers.add(hover); return hover;
    }

    private void waitForTicket() throws Exception { waitUntil(() -> { try { return count() == 1; } catch (Exception e) { throw new AssertionError(e); } }); }
    private DocumentationData result(Hover hover) throws Exception {
        waitUntil(() -> hover.finished().getCount() == 0); return hover.future().get(1, TimeUnit.SECONDS);
    }

    public void testEightTicketsCoalesceAndOneCancellationDoesNotCancelOthers() throws Exception {
        blocked();
        var signals = new ArrayList<CompletableFuture<Boolean>>();
        Object lock = serviceLock();
        for (int i = 0; i < NagiAssistService.MAX_DOCUMENTATION_TICKETS; i++) {
            var signal = ticket(); signal.thenRun(() -> assertFalse("notify must run outside the service lock", Thread.holdsLock(lock)));
            signals.add(signal);
        }
        assertEquals(8, count());
        assertEquals(Boolean.FALSE, ticket().getNow(null));
        waitUntil(() -> starts.get() == 1);
        signals.removeFirst().cancel(false);
        assertEquals(7, count());
        signals.add(ticket()); assertEquals(8, count());
        assertFalse(cancellation.get().isCancelled());
        release.countDown();
        waitUntil(() -> signals.stream().allMatch(Future::isDone));
        for (var signal : signals) assertEquals(Boolean.TRUE, signal.get());
        assertEquals(1, starts.get()); assertEquals(0, count());
        assertEquals("registration after publication cannot lose its wakeup", Boolean.TRUE, ticket().getNow(null));
        assertEquals(0, count());
    }

    public void testTimeoutExpiresWhileTheSingleWorkerIsBlocked() throws Exception {
        NagiSettings.update(NagiSettings.getInstance().getState().compilerPath, 1);
        blocked();
        var signal = ticket();
        waitUntil(() -> starts.get() == 1);
        waitUntil(signal::isDone);
        assertEquals(Boolean.FALSE, signal.get()); assertEquals(0, count());
        assertEquals("the timeout must not need the blocked worker", 1, release.getCount());
        assertFalse("timing out one hover leaves shared work alone", cancellation.get().isCancelled());
    }

    public void testFailureAndRejectedAdmissionReleaseTicketsImmediately() throws Exception {
        install((plan, input, timeout, cancelled, guard) -> { starts.incrementAndGet(); throw new java.io.IOException("controlled"); });
        var signal = ticket(); waitUntil(signal::isDone);
        assertEquals(Boolean.FALSE, signal.get()); assertEquals(0, count());
        assertEquals(Boolean.FALSE, ticket().getNow(null)); assertEquals(0, count()); assertEquals(1, starts.get());
    }

    public void testSameSnapshotCompletionReplacementKeepsDocumentationTicket() throws Exception {
        CountDownLatch old = new CountDownLatch(1);
        AtomicReference<NagiAssistProcess.Cancellation> first = new AtomicReference<>();
        install((plan, input, timeout, cancelled, guard) -> {
            if (starts.incrementAndGet() == 1) {
                first.set(cancelled);
                while (old.getCount() > 0) try { old.await(); } catch (InterruptedException ignored) { }
            }
            return facts();
        });
        try {
            var signal = ticket(); waitUntil(() -> starts.get() == 1);
            myFixture.getEditor().getCaretModel().moveToOffset(file.getText().lastIndexOf("add(") + 2);
            service.requestCompletion(file, myFixture.getEditor());
            assertTrue(first.get().isCancelled()); assertFalse(signal.isDone());
            old.countDown(); waitUntil(signal::isDone);
            assertEquals(Boolean.TRUE, signal.get()); assertEquals(2, starts.get()); assertEquals(0, count());
        } finally { old.countDown(); }
    }

    public void testDifferentFileReplacementRejectsTheOldGroup() throws Exception {
        var other = open("other.nagi", "def main():\n    print(2)\n");
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        blocked(); var signal = ticket(); waitUntil(() -> starts.get() == 1);
        service.requestBaseline(other);
        assertEquals(Boolean.FALSE, signal.getNow(null));
        assertEquals(0, count());
    }

    public void testOldPointerAndUnknownOffsetsCannotUseNewCompilerFacts() throws Exception {
        install((plan, input, timeout, cancelled, guard) -> facts());
        var signal = ticket(); waitUntil(signal::isDone); assertEquals(Boolean.TRUE, signal.get());
        var editor = myFixture.getEditor();
        long stamp = document.getModificationStamp(), generation = service.documentationEpoch();
        var pointer = NagiPlatform.read(() -> {
            var provider = com.intellij.lang.documentation.ide.IdeDocumentationTargetProvider.getInstance(getProject());
            assertTrue(new NagiDocumentationTargetProvider().documentationTargets(file, -1).isEmpty());
            assertTrue(new NagiDocumentationTargetProvider().documentationTargets(file, file.getTextLength()).isEmpty());
            assertTrue(new NagiDocumentationTargetProvider().documentationTargets(file, 3).isEmpty());
            return provider.documentationTargets(editor, file, file.getText().lastIndexOf("add(") + 1).getFirst().createPointer();
        });
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(
                document.getText().lastIndexOf('1'), document.getText().lastIndexOf('1') + 1, "2"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        assertEquals(Boolean.FALSE, NagiPlatform.read(() -> service.documentationTicket(file, document, stamp, generation)).getNow(null));
        var current = ticket(); waitUntil(current::isDone); assertEquals(Boolean.TRUE, current.get());
        assertNull("a pointer captured at the same spelling/offset cannot authorize a new generation", NagiPlatform.read(pointer::dereference));
    }

    private NagiAssistProtocol.Response factsWithDependency(PsiFile dependency) {
        var response = facts();
        return new NagiAssistProtocol.Response(response.semanticStatus(), true, true, false, "names", List.of(), List.of(),
                response.references(), List.of(file.getVirtualFile().getPath(), dependency.getVirtualFile().getPath()),
                response.definitions(), List.of());
    }

    public void testRecaptureKeepsItsDeadlineAndLateFinallyCannotEraseTheReplacement() throws Exception {
        var dependency = open("captured.nagi", "def helper() -> i64:\n    return 1\n");
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        CountDownLatch second = new CountDownLatch(1);
        install((plan, input, timeout, cancelled, guard) -> {
            if (starts.incrementAndGet() == 2) {
                while (second.getCount() > 0) try { second.await(); } catch (InterruptedException ignored) { }
            }
            return factsWithDependency(dependency);
        });
        try {
            var first = ticket(); waitUntil(() -> starts.get() == 2);
            assertFalse("first dependency response was discarded, not published", first.isDone());
            assertEquals(1, count());
            int literal = document.getText().lastIndexOf('1');
            WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(literal, literal + 1, "2"));
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertEquals(Boolean.FALSE, first.getNow(null));
            var replacement = ticket();
            assertFalse(replacement.isDone());
            second.countDown(); waitUntil(replacement::isDone);
            assertEquals(Boolean.TRUE, replacement.get());
            assertEquals(3, starts.get()); assertEquals(0, count());
        } finally { second.countDown(); }
    }

    public void testRecaptureEnqueueRejectionAndClosedWorkerReleaseTickets() throws Exception {
        var dependency = open("rejected.nagi", "def helper() -> i64:\n    return 1\n");
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        var workerField = NagiAssistService.class.getDeclaredField("worker"); workerField.setAccessible(true);
        install((plan, input, timeout, cancelled, guard) -> {
            starts.incrementAndGet();
            ((java.util.concurrent.ScheduledThreadPoolExecutor) workerField.get(service)).shutdown();
            return factsWithDependency(dependency);
        });
        var signal = ticket(); waitUntil(signal::isDone);
        assertEquals(Boolean.FALSE, signal.get()); assertEquals(0, count()); assertEquals(1, starts.get());
        assertEquals(Boolean.FALSE, ticket().getNow(null)); assertEquals(0, count());
    }

    public void testEditInvalidatesTheSamePendingPlatformHover() throws Exception {
        blocked(); var hover = hover(); waitForTicket();
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.insertString(0, "# edited\n"));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        assertNull(result(hover)); assertEquals(0, count()); assertNull(service.fresh(file));
    }

    public void testDependencyEditInvalidatesPendingHover() throws Exception {
        var dependency = open("helper.nagi", "def helper() -> i64:\n    return 1\n");
        var dependencyDocument = PsiDocumentManager.getInstance(getProject()).getDocument(dependency);
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        blocked(); var hover = hover(); waitForTicket(); waitUntil(() -> starts.get() == 1);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> dependencyDocument.insertString(0, "# changed\n"));
        assertNull(result(hover)); assertEquals(0, count());
    }

    public void testTrustRevocationImmediatelyReleasesPendingPlatformHover() throws Exception {
        blocked(); var hover = hover(); waitForTicket();
        TrustedProjects.setProjectTrusted(getProject(), false);
        assertNull(result(hover)); assertEquals(0, count());
        assertEquals(Boolean.FALSE, ticket().getNow(null));
    }

    public void testSettingsChangeImmediatelyReleasesPendingPlatformHover() throws Exception {
        blocked(); var hover = hover(); waitForTicket();
        NagiSettings.update(NagiSettings.getInstance().getState().compilerPath, 6);
        assertNull(result(hover)); assertEquals(0, count());
    }

    public void testCloseAndDisposeReleasePendingTickets() throws Exception {
        blocked(); var first = ticket();
        FileEditorManager.getInstance(getProject()).closeFile(file.getVirtualFile());
        assertEquals(Boolean.FALSE, first.getNow(null)); assertEquals(0, count());
        myFixture.configureFromExistingVirtualFile(file.getVirtualFile());
        var second = ticket(); service.dispose();
        assertEquals(Boolean.FALSE, second.getNow(null)); assertEquals(0, count());
        assertEquals(Boolean.FALSE, ticket().getNow(null));
        TrustedProjects.setProjectTrusted(getProject(), false);
        assertEquals("a disposed service must ignore later AppLevel trust events", NagiAssistService.State.CLOSED, service.state());
    }

    public void testCancellingPlatformConsumerUnregistersOnlyItsOwnTicket() throws Exception {
        blocked(); var hover = hover(); waitForTicket(); waitUntil(() -> starts.get() == 1);
        var other = ticket();
        assertNotNull(hover.job().get());
        hover.job().get().cancel(new java.util.concurrent.CancellationException("cancel this hover"));
        waitUntil(() -> hover.finished().getCount() == 0);
        try { hover.future().get(); fail("platform cancellation must propagate"); }
        catch (java.util.concurrent.ExecutionException failure) {
            assertTrue(failure.getCause().toString(), failure.getCause() instanceof java.util.concurrent.CancellationException
                    || failure.getCause() instanceof com.intellij.openapi.progress.ProcessCanceledException);
        }
        assertEquals(1, count()); assertFalse(other.isDone()); assertFalse(cancellation.get().isCancelled());
        release.countDown(); waitUntil(other::isDone); assertEquals(Boolean.TRUE, other.get());
    }
}
