package com.disnana.nagi;

import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.Disposable;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.openapi.vfs.VirtualFileManager;
import com.intellij.openapi.vfs.newvfs.BulkFileListener;
import com.intellij.openapi.vfs.newvfs.events.VFileCreateEvent;
import com.intellij.openapi.vfs.newvfs.events.VFileEvent;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiFile;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.util.ui.UIUtil;
import com.intellij.util.messages.MessageBus;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BooleanSupplier;
import java.util.function.Supplier;

/** Real Platform documents, background snapshots and deterministic cancellation gates. */
public class NagiAssistServiceTest extends BasePlatformTestCase {
    private Path root;
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

    public void testRustVerbatimWindowsSourcePathsHaveTheSameIdeIdentity() {
        assertEquals(NagiAssistService.normalizePath("C:\\work\\main.nagi"),
                NagiAssistService.normalizePath("\\\\?\\C:\\work\\main.nagi"));
        assertEquals(NagiAssistService.normalizePath("\\\\server\\share\\main.low"),
                NagiAssistService.normalizePath("\\\\?\\UNC\\server\\share\\main.low"));
    }
    private void waitUntil(BooleanSupplier done) throws Exception {
        long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
        while (!done.getAsBoolean() && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents();
            Thread.sleep(5);
        }
        assertTrue("service state: " + service.state() + ": " + service.statusMessage(), done.getAsBoolean());
    }
    private void waitUntil(BooleanSupplier done, Supplier<String> failureMessage) throws Exception {
        long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
        while (!done.getAsBoolean() && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents();
            Thread.sleep(5);
        }
        boolean complete = done.getAsBoolean();
        assertTrue(complete ? "" : failureMessage.get(), complete);
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
        PsiFile file = source("main.nagi", "def main():\n    print(1)\n");
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

        service.requestBaseline(file);
        waitUntil(() -> service.state() == NagiAssistService.State.COMPILER_UNAVAILABLE && notices.get() == 1);
        assertEquals("initial resolution attempt", 1, compilerProbes.get());
        assertEquals(0, calls.get());
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
}
