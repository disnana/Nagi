package com.disnana.nagi;

import com.intellij.codeInsight.AutoPopupController;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer;
import com.intellij.codeInsight.hint.ShowParameterInfoHandler;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.notification.Notification;
import com.intellij.notification.NotificationAction;
import com.intellij.notification.NotificationGroupManager;
import com.intellij.notification.NotificationType;
import com.intellij.notification.Notifications;
import com.intellij.openapi.Disposable;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.application.ReadAction;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.editor.event.DocumentEvent;
import com.intellij.openapi.editor.event.DocumentListener;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.fileEditor.FileEditorManager;
import com.intellij.openapi.fileEditor.FileEditorManagerEvent;
import com.intellij.openapi.fileEditor.FileEditorManagerListener;
import com.intellij.openapi.options.ShowSettingsUtil;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.util.Disposer;
import com.intellij.openapi.util.SystemInfo;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.openapi.vfs.VfsUtil;
import com.intellij.openapi.vfs.VirtualFile;
import com.intellij.openapi.vfs.VirtualFileManager;
import com.intellij.openapi.vfs.newvfs.BulkFileListener;
import com.intellij.openapi.vfs.newvfs.events.VFileEvent;
import com.intellij.openapi.vfs.newvfs.events.VFileMoveEvent;
import com.intellij.openapi.vfs.newvfs.events.VFilePropertyChangeEvent;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.PsiManager;
import com.intellij.util.messages.MessageBusConnection;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.InvalidPathException;
import java.nio.file.Path;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import org.jetbrains.annotations.NotNull;

/** Project-scoped, trust-gated snapshot runner for compiler-authoritative editor facts. */
public final class NagiAssistService implements Disposable {
    private static final int MAX_TRACKED_FILES = 256;
    private static final int MAX_DIRECTORY_ENTRIES = 8192;
    private static final long DIAGNOSTIC_DEBOUNCE_MS = 350;
    private static final long COMPLETION_DEBOUNCE_MS = 80;
    private static final String COMPILER_UNAVAILABLE_MESSAGE =
            "The configured Nagi compiler could not be resolved to an executable file.";
    private static final String COMPILER_NOTIFICATION_GROUP = "Nagi compiler assistance";

    public enum State { IDLE, WAITING, RUNNING, READY, UNTRUSTED, UNSAVED_MANIFEST, LIMIT, COMPILER_UNAVAILABLE, FAILED, STALE, CLOSED }

    public static final class CachedResponse {
        private final Snapshot snapshot;
        private final NagiAssistProtocol.Response response;
        private final Set<String> sourceFileKeys;
        private final Map<ReferenceKey, NagiAssistProtocol.Location> referenceTargets;
        private final NagiSemanticIndex semanticIndex;
        private final Object presentationIdentity = new Object();
        private CachedResponse(Snapshot snapshot, NagiAssistProtocol.Response response) {
            this.snapshot = snapshot;
            this.response = response;
            LinkedHashSet<String> files = new LinkedHashSet<>();
            for (String source : response.sourceFiles()) files.add(normalizePath(source));
            for (NagiAssistProtocol.Diagnostic diagnostic : response.diagnostics()) {
                if (diagnostic.file() != null && !diagnostic.file().isBlank()) files.add(normalizePath(diagnostic.file()));
            }
            sourceFileKeys = Set.copyOf(files);
            Map<ReferenceKey, NagiAssistProtocol.Location> references = new HashMap<>();
            for (NagiAssistProtocol.Reference reference : response.references()) {
                NagiAssistProtocol.Location location = reference.location();
                references.put(new ReferenceKey(normalizePath(location.file()), location.line(), location.column(), location.length()), reference.target());
            }
            referenceTargets = Map.copyOf(references);
            semanticIndex = new NagiSemanticIndex(response);
        }
        public NagiAssistProtocol.Response response() { return response; }
    }

    private record Position(int line, int column, long offset) {}
    private record Intent(VirtualFile file, Document document, long documentStamp, long epoch,
                          Position position, Editor editor) {
        String key() {
            return normalizePath(file.getPath()) + ":" + documentStamp + ":" + epoch + ":"
                    + (position == null ? "-" : position.line() + ":" + position.column());
        }
    }
    private record RawFile(VirtualFile file, Document document, String path, long documentStamp,
                           long fileStamp, boolean unsaved, String text) {}
    private record RawCapture(VirtualFile focusFile, Document focusDocument, long focusStamp,
                              long focusFileStamp, List<RawFile> files, String projectBase,
                              String compilerSetting, int timeout, long epoch, Position position,
                              Editor editor, String activePath, boolean exceedsOverlayLimit,
                              boolean exceedsTrackedFileLimit) {}
    private record DocumentStamp(Document document, long stamp) {}
    private record FileStamp(VirtualFile file, long stamp) {}
    private record DiskStamp(Path path, byte[] contents) {}
    private record DirectoryEntry(String name, String kind) {}
    private record DirectoryStamp(Path path, List<DirectoryEntry> entries, boolean incomplete,
                                  java.nio.file.attribute.FileTime modified, String fileKey) {}
    private record ExecutableStamp(VirtualFile file, long virtualStamp, long size,
                                   long modifiedMillis, String fileKey) {}
    private record CompilerSelection(String configuredPath, String projectBase) {}
    private record CompilerUnavailable(CompilerSelection selection, Set<String> candidatePaths) {}
    private record ReferenceKey(String file, int line, int column, int length) {}
    private record Snapshot(long epoch, String requestKey, String compilerSetting, int timeout,
                           String projectBase, Path scopePath, Path focusPath, Path executable,
                           ExecutableStamp executableStamp, NagiAssistCommandPlan plan,
                           NagiAssistProtocol.Query query, byte[] input,
                           List<DocumentStamp> documents, List<FileStamp> files, List<DiskStamp> disks,
                           List<DirectoryStamp> directories,
                           Map<String, String> canonicalPaths, List<VirtualFile> openSources,
                           Document focusDocument, long focusDocumentStamp, VirtualFile focusFile,
                           long focusFileStamp, Editor editor, long caretOffset) {
        private Snapshot { input = input.clone(); }
    }
    private enum Admission { CURRENT, JOINED, SCHEDULED, REJECTED }
    private record DocumentationTicket(Document document, long stamp, long epoch, String group,
                                       java.util.concurrent.CompletableFuture<Boolean> signal,
                                       java.util.concurrent.CompletableFuture<Boolean> deadline, Object owner) {
        DocumentationTicket ownedBy(Object owner) { return new DocumentationTicket(document, stamp, epoch, group, signal, deadline, owner); }
    }
    private record DocumentationRecapture(Object attempt, VirtualFile file, Document document, long stamp, long epoch, String group) {}
    private final List<DocumentationTicket> documentationTickets = new ArrayList<>();
    static final int MAX_DOCUMENTATION_TICKETS = 8;

    private enum PopupKind { COMPLETION, PARAMETERS }
    private record PopupWaiter(Editor editor, Document document, VirtualFile file, long stamp, long offset,
                               PopupKind kind, boolean respectParameterSetting) {}

    private final Project project;
    private final ScheduledThreadPoolExecutor worker;
    private final CompilerUnavailablePresenter compilerUnavailablePresenter;
    private final CompilerResolver compilerResolver;
    private final AtomicLong epoch = new AtomicLong();
    private final Object lock = new Object();
    private final IdentityHashMap<Document, ListenerRecord> documentListeners = new IdentityHashMap<>();
    private final LinkedHashSet<String> popupScheduled = new LinkedHashSet<>();
    private final NagiAssistProcess.Runner runner;
    private NagiAssistProcess.Session session;
    private volatile State state = State.IDLE;
    private volatile String statusMessage = "";
    private volatile CachedResponse cached;
    private final NagiSemanticPresentation presentation = new NagiSemanticPresentation();
    private volatile Set<String> knownSourcePaths = Set.of();
    private volatile Set<String> knownDependencies = Set.of();
    // Discovery hints only: candidate meaning remains entirely compiler-owned.
    private volatile Set<String> knownImportDirectories = Set.of();
    private volatile String importQueryFile;
    // Confined to the existing worker, including registration and removal.
    private LocalFileSystem.WatchRequest importDirectoryWatch;
    private String watchedImportDirectory;
    private VirtualFile watchedImportVirtualDirectory;
    private boolean importDirectoryPrimed;
    private volatile VirtualFile lastFocusFile;
    private ScheduledFuture<?> pending;
    private ScheduledFuture<?> activeFuture;
    private NagiAssistProcess.Cancellation activeCancellation;
    private String activeKey;
    private Object activeAttempt;
    private String pendingKey;
    private String failedKey;
    private volatile CompilerSelection unavailableCompilerSelection;
    private volatile CompilerSelection presentedUnavailableSelection;
    private volatile Set<String> unavailableCompilerPaths = Set.of();
    private List<PopupWaiter> pendingWaiters = List.of();
    private List<PopupWaiter> activeWaiters = List.of();
    private volatile boolean disposed;

    public NagiAssistService(Project project) { this(project, new NagiAssistProcess.Session()); }

    private NagiAssistService(Project project, NagiAssistProcess.Session session) {
        this(project, session::run, NagiAssistService::showCompilerUnavailable);
        this.session = session;
    }

    NagiAssistService(Project project, NagiAssistProcess.Runner runner) {
        this(project, runner, NagiAssistService::showCompilerUnavailable);
    }

    NagiAssistService(Project project, NagiAssistProcess.Runner runner,
                      CompilerUnavailablePresenter compilerUnavailablePresenter) {
        this(project, runner, compilerUnavailablePresenter, NagiAssistService::resolveCompiler);
    }

    NagiAssistService(Project project, NagiAssistProcess.Runner runner,
                      CompilerUnavailablePresenter compilerUnavailablePresenter,
                      CompilerResolver compilerResolver) {
        this.project = project;
        this.runner = runner;
        this.compilerUnavailablePresenter = compilerUnavailablePresenter;
        this.compilerResolver = compilerResolver;
        worker = new ScheduledThreadPoolExecutor(1, runnable -> {
            Thread thread = new Thread(runnable, "Nagi editor assistance");
            thread.setDaemon(true);
            return thread;
        });
        worker.setRemoveOnCancelPolicy(true);
        ApplicationManager.getApplication().getMessageBus().connect(this).subscribe(
                com.intellij.ide.trustedProjects.TrustedProjectsListener.TOPIC,
                new com.intellij.ide.trustedProjects.TrustedProjectsListener() {
                    @Override public void onProjectUntrusted(@NotNull Project changed) {
                        if (changed == project && !disposed && !project.isDisposed()) {
                            invalidate(State.UNTRUSTED, "Project trust is required before the Nagi compiler can run.");
                            restartOpenSourcesLater();
                        }
                    }
                });
        MessageBusConnection projectConnection = project.getMessageBus().connect(this);
        projectConnection.subscribe(FileEditorManagerListener.FILE_EDITOR_MANAGER, new FileEditorManagerListener() {
            @Override public void fileOpened(@NotNull FileEditorManager source, @NotNull VirtualFile file) {
                if (isTrackedSource(file) || isManifest(file)) {
                    invalidate(State.STALE, "An Nagi source or manifest changed its open state.");
                    enqueue(() -> { attachOpenDocuments(); restartOpenSourcesLater(); });
                }
            }
            @Override public void fileClosed(@NotNull FileEditorManager source, @NotNull VirtualFile file) {
                if (isTrackedSource(file) || isManifest(file)) {
                    if (lastFocusFile == file) lastFocusFile = null;
                    detachDocument(file);
                    invalidate(State.STALE, "An Nagi source or manifest changed its open state.");
                    enqueue(NagiAssistService.this::attachOpenDocuments);
                }
            }
            @Override public void selectionChanged(@NotNull FileEditorManagerEvent event) {
                // A different editor must request its own query; this does not reuse the old cursor.
            }
        });
        projectConnection.subscribe(VirtualFileManager.VFS_CHANGES, new BulkFileListener() {
            @Override public void before(@NotNull List<? extends VFileEvent> events) {
                // Capture removal from an old parent before nested moves/renames
                // update the VFS objects used to describe that parent.
                if (events.stream().filter(event -> event instanceof VFileMoveEvent
                        || event instanceof VFilePropertyChangeEvent change
                        && VirtualFile.PROP_NAME.equals(change.getPropertyName()))
                        .anyMatch(NagiAssistService.this::isRelevantVfsEvent)) {
                    invalidate(State.STALE, "An Nagi source or import directory is moving or being renamed.");
                    restartOpenSourcesLater();
                }
            }
            @Override public void after(@NotNull List<? extends VFileEvent> events) {
                boolean compilerChanged = events.stream().anyMatch(NagiAssistService.this::isUnavailableCompilerVfsEvent);
                if (compilerChanged) clearUnavailableCompiler();
                if (compilerChanged || events.stream().anyMatch(NagiAssistService.this::isRelevantVfsEvent)) {
                    invalidate(State.STALE, "An Nagi source, project manifest, or compiler file changed.");
                    restartOpenSourcesLater();
                }
            }
        });
        ApplicationManager.getApplication().getMessageBus().connect(this)
                .subscribe(NagiSettings.SETTINGS_CHANGED, () -> {
                    CompilerSelection unavailable = unavailableCompilerSelection;
                    if (unavailable != null && !unavailable.equals(currentCompilerSelection())) {
                        clearUnavailableCompiler();
                    }
                    invalidate(State.STALE, "Nagi compiler settings changed.");
                    restartOpenSourcesLater();
                });
    }

    public State state() { return state; }
    public String statusMessage() { return statusMessage; }

    /** Returns only a current compiler response; this method never waits for a process. */
    public CachedResponse fresh(PsiFile file) {
        CachedResponse response = cached;
        if (response == null || !snapshotCurrent(response.snapshot) || !appliesToFile(response, file)) return null;
        return response;
    }

    /** A completion response is reusable only at the same current file and UTF-16 cursor. */
    public CachedResponse fresh(PsiFile file, Editor editor) {
        CachedResponse response = fresh(file);
        if (response == null || editor == null || response.snapshot.query == null) return null;
        if (editor.isDisposed() || editor.getDocument() != response.snapshot.focusDocument) return null;
        Position position = position(editor);
        if (position == null) return null;
        String queryFile = canonicalPathFor(response.snapshot, file.getVirtualFile());
        NagiAssistProtocol.Query query = response.snapshot.query;
        if (!samePath(query.file(), queryFile) || query.line() != position.line() || query.column() != position.column()) return null;
        return response;
    }

    public boolean isCurrent(PsiFile file, CachedResponse response) {
        return response != null && response == cached && snapshotCurrent(response.snapshot) && appliesToFile(response, file);
    }

    public void requestBaseline(PsiFile file) {
        CachedResponse current = fresh(file);
        if (current != null) return;
        request(file, null, null, DIAGNOSTIC_DEBOUNCE_MS);
    }

    /** Shares the active-cursor query with diagnostics so one request serves both editor features. */
    public void requestBaseline(PsiFile file, Editor editor) {
        CachedResponse current = fresh(file);
        if (current != null) return;
        request(file, editor, null, DIAGNOSTIC_DEBOUNCE_MS);
    }

    public void requestNavigation(PsiFile file) { documentationAdmission(file); }

    private Admission documentationAdmission(PsiFile file) {
        if (fresh(file) != null) return Admission.CURRENT;
        String sourcePrefix;
        try { sourcePrefix = normalizePath(file.getVirtualFile().getPath()) + ":"; }
        catch (RuntimeException exception) { return Admission.REJECTED; }
        synchronized (lock) {
            if ((activeKey != null && activeKey.startsWith(sourcePrefix))
                    || (pendingKey != null && pendingKey.startsWith(sourcePrefix))) return Admission.JOINED;
        }
        return request(file, null, null, DIAGNOSTIC_DEBOUNCE_MS);
    }

    long documentationEpoch() { return epoch.get(); }

    /** Called in a short read action. Signals readiness only, never transports semantic facts. */
    java.util.concurrent.CompletableFuture<Boolean> documentationTicket(PsiFile file, Document document,
                                                                       long stamp, long generation) {
        ApplicationManager.getApplication().assertReadAccessAllowed();
        if (!documentationIdentityCurrent(file, document, stamp, generation))
            return java.util.concurrent.CompletableFuture.completedFuture(false);
        var signal = new java.util.concurrent.CompletableFuture<Boolean>();
        var deadline = new java.util.concurrent.CompletableFuture<Boolean>();
        String group = normalizePath(file.getVirtualFile().getPath()) + ":" + stamp + ":" + generation + ":";
        synchronized (lock) {
            if (!documentationIdentityCurrent(file, document, stamp, generation))
                return java.util.concurrent.CompletableFuture.completedFuture(false);
            // The read lock precedes the service lock; publication may have won
            // since the caller's initial fresh check. No first read is acquired here.
            if (fresh(file) != null) return java.util.concurrent.CompletableFuture.completedFuture(true);
            if (documentationTickets.size() >= MAX_DOCUMENTATION_TICKETS)
                return java.util.concurrent.CompletableFuture.completedFuture(false);
            documentationTickets.add(new DocumentationTicket(document, stamp, generation, group, signal, deadline,
                    activeKey != null && activeKey.startsWith(group) ? activeAttempt : null));
        }
        // Callbacks retain only the private deadline identity, never source
        // facts, a Document, or the public signal itself.
        signal.whenComplete((ready, failure) -> discardDocumentationDeadline(deadline));
        long timeout = Math.max(1_350L, Math.min(300_350L,
                1_000L * NagiSettings.getInstance().getState().checkTimeoutSeconds + DIAGNOSTIC_DEBOUNCE_MS));
        // JDK's independent delayed executor also expires a stuck single worker.
        deadline.thenRun(() -> finishDocumentation(deadline, false));
        deadline.completeOnTimeout(false, timeout, TimeUnit.MILLISECONDS);
        try {
            if (!signal.isDone()) {
                Admission admission = documentationAdmission(file);
                if (admission == Admission.CURRENT || admission == Admission.REJECTED) {
                    finishDocumentation(deadline, admission == Admission.CURRENT);
                }
            }
        } catch (RuntimeException | Error failure) {
            finishDocumentation(deadline, false);
            throw failure;
        }
        return signal;
    }

    boolean documentationIdentityCurrent(PsiFile file, Document document, long stamp, long generation) {
        return !disposed && !project.isDisposed() && trustedForCompiler() && file.isValid()
                && generation == epoch.get() && document.getModificationStamp() == stamp
                && PsiDocumentManager.getInstance(project).getDocument(file) == document;
    }

    void unregisterDocumentation(java.util.concurrent.CompletableFuture<Boolean> signal) {
        List<DocumentationTicket> detached;
        synchronized (lock) { detached = detachDocumentation(ticket -> ticket.signal() == signal); }
        detached.forEach(ticket -> ticket.deadline().cancel(false));
    }

    private void discardDocumentationDeadline(java.util.concurrent.CompletableFuture<Boolean> deadline) {
        synchronized (lock) { detachDocumentation(ticket -> ticket.deadline() == deadline); }
        deadline.cancel(false);
    }

    private void finishDocumentation(java.util.concurrent.CompletableFuture<Boolean> deadline, boolean ready) {
        List<DocumentationTicket> detached;
        synchronized (lock) { detached = detachDocumentation(ticket -> ticket.deadline() == deadline); }
        // Only the thread which detached this identity owns its terminal signal.
        notifyDocumentation(detached, ready);
    }

    private List<DocumentationTicket> detachDocumentation(java.util.function.Predicate<DocumentationTicket> predicate) {
        // All callers own lock. Complete the returned scalar signals after leaving it.
        List<DocumentationTicket> detached = new ArrayList<>();
        documentationTickets.removeIf(ticket -> {
            if (!predicate.test(ticket)) return false;
            detached.add(ticket); return true;
        });
        return detached;
    }

    private static void notifyDocumentation(List<DocumentationTicket> tickets, boolean ready) {
        tickets.forEach(ticket -> {
            ticket.deadline().cancel(false);
            ticket.signal().complete(ready);
        });
    }

    public void requestCompletion(PsiFile file, Editor editor) {
        if (editor == null || editor.isDisposed()) return;
        request(file, editor, editor, COMPLETION_DEBOUNCE_MS);
    }

    /** Resume the platform's already requested hint, using the same bounded snapshot worker. */
    public void requestParameterInfo(PsiFile file, Editor editor) {
        if (editor == null || editor.isDisposed()) return;
        request(file, editor, editor, COMPLETION_DEBOUNCE_MS, PopupKind.PARAMETERS);
    }

    public boolean responseContainsFile(CachedResponse response, VirtualFile file) {
        if (response == null || response != cached || file == null || !snapshotCurrent(response.snapshot)) return false;
        String canonical = canonicalPathFor(response.snapshot, file);
        if (samePath(response.snapshot.focusPath.toString(), canonical)) return true;
        return response.sourceFileKeys.contains(normalizePath(canonical));
    }

    public boolean diagnosticTargetsFile(CachedResponse response, VirtualFile file, String diagnosticPath) {
        if (response == null || response != cached || file == null || !snapshotCurrent(response.snapshot)) return false;
        if (diagnosticPath == null || diagnosticPath.isBlank()) {
            return samePath(response.snapshot.focusPath.toString(), canonicalPathFor(response.snapshot, file));
        }
        String canonical = canonicalPathFor(response.snapshot, file);
        return samePath(diagnosticPath, canonical)
                || (!response.snapshot.canonicalPaths.containsKey(normalizePath(diagnosticPath))
                    && samePath(response.snapshot.focusPath.toString(), canonical));
    }

    public boolean diagnosticOriginMatchesFile(CachedResponse response, VirtualFile file, String diagnosticPath) {
        return response != null && response == cached && file != null
                && snapshotCurrent(response.snapshot) && diagnosticPath != null
                && samePath(diagnosticPath, canonicalPathFor(response.snapshot, file));
    }

    /** Exact compiler reference lookup; PSI token shape never determines ownership or target. */
    public NagiAssistProtocol.Location referenceTarget(PsiFile file, PsiElement element) {
        return referenceTarget(file, element, fresh(file));
    }

    private NagiAssistProtocol.Location referenceTarget(PsiFile file, PsiElement element, CachedResponse response) {
        if (!isCurrent(file, response) || element == null || !element.isValid()
                || element.getContainingFile() != file) return null;
        var documents = PsiDocumentManager.getInstance(project);
        Document document = documents.getDocument(file);
        if (document == null || !documents.isCommitted(document)) return null;
        int start = element.getTextRange().getStartOffset();
        int end = element.getTextRange().getEndOffset();
        if (start < 0 || end < start || end > document.getTextLength()) return null;
        int line = document.getLineNumber(start);
        int column = start - document.getLineStartOffset(line);
        String path = canonicalPathFor(response.snapshot, file.getVirtualFile());
        return response.referenceTargets.get(new ReferenceKey(normalizePath(path), line + 1, column + 1, end - start));
    }

    /** A cached PSI reference is only a host; capture one response for lookup and resolution. */
    PsiElement resolveReference(PsiFile file, PsiElement element) {
        CachedResponse response = fresh(file);
        if (response == null) {
            requestNavigation(file); // Nonblocking normal worker admission, never a UI wait.
            return null;
        }
        NagiAssistProtocol.Location target = referenceTarget(file, element, response);
        PsiElement resolved = resolveTarget(file, response, target);
        return isCurrent(file, response) ? resolved : null;
    }

    public NagiAssistProtocol.Definition functionDefinition(PsiFile file, PsiElement callee) {
        CachedResponse response = fresh(file);
        if (response == null || callee == null || !callee.isValid()) return null;
        Document document = PsiDocumentManager.getInstance(project).getDocument(file);
        if (document == null) return null;
        int start = callee.getTextRange().getStartOffset();
        int end = callee.getTextRange().getEndOffset();
        if (start < 0 || end <= start || end > document.getTextLength()) return null;
        int line = document.getLineNumber(start);
        String path = canonicalPathFor(response.snapshot, file.getVirtualFile());
        return response.semanticIndex.function(path, line + 1,
                start - document.getLineStartOffset(line) + 1, end - start);
    }

    /** Documentation uses exact compiler spans, including declaration names and virtual stdlib targets. */
    public NagiAssistProtocol.Definition documentedDefinition(PsiFile file, PsiElement element) {
        CachedResponse response = fresh(file);
        if (response == null || element == null || !element.isValid() || element.getContainingFile() != file) return null;
        Document document = PsiDocumentManager.getInstance(project).getDocument(file);
        if (document == null) return null;
        int start = element.getTextRange().getStartOffset();
        int end = element.getTextRange().getEndOffset();
        if (start < 0 || end <= start || end > document.getTextLength()) return null;
        int line = document.getLineNumber(start);
        if (end > document.getLineEndOffset(line)) return null;
        String path = canonicalPathFor(response.snapshot, file.getVirtualFile());
        return response.semanticIndex.definition(path, line + 1,
                start - document.getLineStartOffset(line) + 1, end - start);
    }

    String documentationHtml(PsiFile file, PsiElement element) {
        CachedResponse response = fresh(file);
        if (response == null || element == null || !element.isValid() || element.getContainingFile() != file) return null;
        var definition = documentedDefinition(file, element);
        String html = definition == null ? null : NagiDocumentation.definition(definition);
        if (html == null) {
            Document document = PsiDocumentManager.getInstance(project).getDocument(file);
            if (document == null) return null;
            int start = element.getTextOffset(), length = element.getTextLength();
            if (start < 0 || length < 1 || start + length > document.getTextLength()) return null;
            int line = document.getLineNumber(start);
            if (start + length > document.getLineEndOffset(line)) return null;
            var local = response.semanticIndex.local(canonicalPathFor(response.snapshot, file.getVirtualFile()),
                    line + 1, start - document.getLineStartOffset(line) + 1, length);
            if (local != null && local.name().equals(element.getText())) html = NagiDocumentation.local(local);
        }
        return isCurrent(file, response) ? html : null;
    }

    public List<NagiSemanticIndex.Color> semanticColors(PsiFile file, CachedResponse response) {
        if (!isCurrent(file, response)) return List.of();
        return response.semanticIndex.colors(canonicalPathFor(response.snapshot, file.getVirtualFile()));
    }

    List<NagiSemanticPresentation.Paint> rolePresentation(PsiFile file, Document document, CachedResponse response) {
        if (disposed || project.isDisposed()) { presentation.clear(); return List.of(); }
        if (!trustedForCompiler()) {
            invalidate(State.UNTRUSTED, "Project trust is required before the Nagi compiler can run.");
            return List.of();
        }
        if (response != null) {
            if (!isCurrent(file, response)) return List.of();
            var colors = semanticColors(file, response);
            // Completion status is not index completeness. Only an accepted
            // frontend plus an explicit index can replace all visual roles.
            return response.response.frontendAccepted() && response.response.hasSymbolIndex()
                    ? presentation.current(file, document, response.presentationIdentity, colors)
                    : presentation.partial(file, document, response.presentationIdentity, colors);
        }
        return presentation.retained(file, document);
    }

    /** Resolve only a compiler-provided path and range in the same loaded source graph. */
    public PsiElement resolveTarget(PsiFile context, CachedResponse response, NagiAssistProtocol.Location target) {
        if (!isCurrent(context, response) || target == null) return null;
        String path = normalizePath(target.file());
        if (!response.sourceFileKeys.contains(path)) return null;
        if (target.length() == 0) {
            if (target.line() != 1 || target.column() != 1) return null;
            String canonicalPath = response.snapshot.canonicalPaths.get(path);
            if (canonicalPath == null || !samePath(path, canonicalPath)) return null;
            Path sourcePath;
            try { sourcePath = Path.of(canonicalPath); }
            catch (InvalidPathException exception) { return null; }
            if (!sourcePath.isAbsolute() || !isNagiSource(sourcePath)) return null;
            VirtualFile sourceFile = LocalFileSystem.getInstance().findFileByPath(canonicalPath);
            if (sourceFile == null || !sourceFile.isValid() || !sourceFile.isInLocalFileSystem()
                    || sourceFile.isDirectory() || !samePath(canonicalPath, sourceFile.getPath())) return null;
            PsiFile importedFile = PsiManager.getInstance(project).findFile(sourceFile);
            if (importedFile == null || !importedFile.isValid()
                    || !responseContainsFile(response, sourceFile)) return null;
            Document importedDocument = FileDocumentManager.getInstance().getCachedDocument(sourceFile);
            if (importedDocument != null && !PsiDocumentManager.getInstance(project).isCommitted(importedDocument)) return null;
            return importedFile;
        }
        if (target.length() < 1) return null;
        String targetPath = response.snapshot.canonicalPaths.get(path);
        if (targetPath == null) return null;
        VirtualFile virtualFile = LocalFileSystem.getInstance().findFileByPath(targetPath);
        if (virtualFile == null || !virtualFile.isValid() || !virtualFile.isInLocalFileSystem()) return null;
        PsiFile targetFile = PsiManager.getInstance(project).findFile(virtualFile);
        if (targetFile == null || !targetFile.isValid() || !responseContainsFile(response, virtualFile)) return null;
        Document document = PsiDocumentManager.getInstance(project).getDocument(targetFile);
        if (document == null || !PsiDocumentManager.getInstance(project).isCommitted(document)) return null;
        int lineIndex = target.line() - 1;
        if (lineIndex < 0 || lineIndex >= document.getLineCount()) return null;
        int lineStart = document.getLineStartOffset(lineIndex);
        int lineEnd = document.getLineEndOffset(lineIndex);
        long startLong = (long) lineStart + target.column() - 1;
        long endLong = startLong + target.length();
        if (startLong < lineStart || endLong > lineEnd || endLong > document.getTextLength()) return null;
        int start = (int) startLong;
        int end = (int) endLong;
        if (splitsSurrogatePair(document, start) || splitsSurrogatePair(document, end)) return null;
        PsiElement element = targetFile.findElementAt(start);
        if (element == null || !element.isValid()) return null;
        if (element.getTextRange().getStartOffset() > start || element.getTextRange().getEndOffset() < end) return null;
        return element;
    }

    private Admission request(PsiFile file, Editor queryEditor, Editor popupEditor, long delayMillis) {
        return request(file, queryEditor, popupEditor, delayMillis, PopupKind.COMPLETION);
    }

    private Admission request(PsiFile file, Editor queryEditor, Editor popupEditor, long delayMillis, PopupKind kind) {
        if (disposed || project.isDisposed()) { setState(State.CLOSED, "Project is closed."); return Admission.REJECTED; }
        if (file == null || !file.isValid() || !NagiCompilerAction.isSourceFile(file.getVirtualFile())) {
            setState(State.STALE, "Open an existing local .nagi or .low file for compiler assistance.");
            return Admission.REJECTED;
        }
        lastFocusFile = file.getVirtualFile();
        if (!trustedForCompiler()) {
            invalidate(State.UNTRUSTED, "Project trust is required before the Nagi compiler can run.");
            return Admission.REJECTED;
        }
        if (compilerUnavailableForCurrentSelection()) return Admission.REJECTED;
        Intent intent;
        try { intent = makeIntent(file, queryEditor); }
        catch (RuntimeException exception) {
            setState(State.FAILED, "Could not capture the current Nagi editor snapshot.");
            return Admission.REJECTED;
        }
        if (intent == null) {
            setState(State.STALE, "The current Nagi editor snapshot is unavailable.");
            return Admission.REJECTED;
        }
        CachedResponse current = cached;
        if (current != null && snapshotCurrent(current.snapshot) && current.snapshot.requestKey.equals(intent.key())) return Admission.CURRENT;
        if (intent.position() == null && current != null && snapshotCurrent(current.snapshot) && appliesToFile(current, file)) return Admission.CURRENT;
        List<DocumentationTicket> displaced;
        Admission admission = Admission.SCHEDULED;
        synchronized (lock) {
            if (disposed || project.isDisposed()) return Admission.REJECTED;
            if (intent.key().equals(failedKey)) return Admission.REJECTED;
            String sourcePrefix = normalizePath(intent.file().getPath()) + ":" + intent.documentStamp() + ":" + intent.epoch() + ":";
            if (intent.position() == null
                    && ((activeKey != null && activeKey.startsWith(sourcePrefix))
                    || (pendingKey != null && pendingKey.startsWith(sourcePrefix)))) return Admission.JOINED;
            if (intent.key().equals(pendingKey) || intent.key().equals(activeKey)) {
                if (popupEditor != null) addPopupWaiter(intent, popupEditor, kind);
                return Admission.JOINED;
            }
            displaced = detachDocumentation(ticket -> !ticket.group().equals(sourcePrefix));
            cancelPendingAndActiveLocked();
            documentationTickets.replaceAll(ticket -> ticket.group().equals(sourcePrefix) ? ticket.ownedBy(null) : ticket);
            pendingKey = intent.key();
            pendingWaiters = popupEditor == null ? List.of() : List.of(waiter(intent, popupEditor, kind));
            setState(State.WAITING, "Waiting for a stable Nagi source snapshot.");
            try {
                pending = worker.schedule(() -> execute(intent), delayMillis, TimeUnit.MILLISECONDS);
            } catch (java.util.concurrent.RejectedExecutionException exception) {
                pendingKey = null;
                pendingWaiters = List.of();
                setState(State.CLOSED, "Project is closed.");
                displaced.addAll(detachDocumentation(ticket -> true));
                admission = Admission.REJECTED;
            }
        }
        notifyDocumentation(displaced, false);
        return admission;
    }

    private Intent makeIntent(PsiFile file, Editor editor) {
        return NagiPlatform.read(() -> {
            if (project.isDisposed() || !file.isValid()) return null;
            VirtualFile virtualFile = file.getVirtualFile();
            Document document = PsiDocumentManager.getInstance(project).getDocument(file);
            if (virtualFile == null || document == null || !virtualFile.isInLocalFileSystem()) return null;
            if (editor != null && (editor.isDisposed() || editor.getDocument() != document)) return null;
            Position position = editor == null ? null : position(editor);
            if (editor != null && position == null) return null;
            return new Intent(virtualFile, document, document.getModificationStamp(), epoch.get(), position, editor);
        });
    }

    private void execute(Intent intent) {
        String key = intent.key();
        ScheduledFuture<?> task;
        Object attempt = new Object();
        DocumentationRecapture continuation = null;
        synchronized (lock) {
            if (disposed || project.isDisposed() || !key.equals(pendingKey)) return;
            task = pending;
            pending = null;
            pendingKey = null;
            activeKey = key;
            activeAttempt = attempt;
            String group = documentationGroup(intent.file(), intent.documentStamp(), intent.epoch());
            documentationTickets.replaceAll(ticket -> ticket.group().equals(group) ? ticket.ownedBy(attempt) : ticket);
            activeFuture = task;
            activeWaiters = pendingWaiters;
            pendingWaiters = List.of();
        }
        try {
            if (intent.epoch() != epoch.get()) {
                restartOpenSourcesLater();
                return;
            }
            if (!trustedForCompiler()) {
                invalidate(State.UNTRUSTED, "Project trust is required before the Nagi compiler can run.");
                return;
            }
            attachOpenDocuments();
            Snapshot snapshot = buildSnapshot(intent, key);
            if (snapshot == null || !snapshotCurrent(snapshot) || !diskSnapshotCurrent(snapshot)) {
                setStateForActive(key, State.STALE, "The Nagi source snapshot changed before compiler analysis.");
                return;
            }
            NagiAssistProcess.Cancellation cancellation = new NagiAssistProcess.Cancellation(
                    System.nanoTime() + TimeUnit.SECONDS.toNanos(Math.max(1, Math.min(300, snapshot.timeout))));
            synchronized (lock) {
                if (disposed || !key.equals(activeKey)) return;
                activeCancellation = cancellation;
            }
            setStateForActive(key, State.RUNNING, "Running the Nagi compiler on an immutable editor snapshot.");
            if (!executableIdentityCurrent(snapshot)) {
                setStateForActive(key, State.STALE, "The configured Nagi compiler changed before it could run.");
                return;
            }
            cancellation.checkDeadline();
            NagiAssistProtocol.Response response = runner.run(snapshot.plan, snapshot.input, snapshot.timeout,
                    cancellation, () -> !cancellation.isCancelled() && trustedForCompiler()
                            && snapshotCurrent(snapshot) && executableIdentityCurrent(snapshot));
            cancellation.checkDeadline();
            if (cancellation.isCancelled() || !snapshotCurrent(snapshot) || !executableIdentityCurrent(snapshot)
                    || !diskSnapshotCurrent(snapshot)) {
                setStateForActive(key, State.STALE, "The Nagi compiler response belongs to an outdated editor snapshot and was discarded.");
                retrySnapshotLater(snapshot);
                return;
            }
            // Published 0.2.1 can parse the real source but lose its entire index
            // when caret-only recovery inserts an illegal top-level expression.
            // Retry only this missing-index case, on the same immutable snapshot.
            // A present-but-empty index and an original parse failure are final.
            if (snapshot.query != null && response.frontendChecked() && !response.hasSymbolIndex()
                    && response.importContext() == null && response.completions().isEmpty()
                    && response.semanticStatus().equals("none")) {
                byte[] baselineInput = NagiAssistProtocol.withoutQuery(snapshot.input);
                cancellation.checkDeadline();
                NagiAssistProtocol.Response baseline = runner.run(snapshot.plan,
                        baselineInput, snapshot.timeout, cancellation,
                        () -> !cancellation.isCancelled() && trustedForCompiler()
                                && snapshotCurrent(snapshot) && executableIdentityCurrent(snapshot));
                cancellation.checkDeadline();
                if (cancellation.isCancelled() || !snapshotCurrent(snapshot) || !executableIdentityCurrent(snapshot)
                        || !diskSnapshotCurrent(snapshot)) {
                    setStateForActive(key, State.STALE, "The queryless Nagi source facts belong to an outdated editor snapshot and were discarded.");
                    retrySnapshotLater(snapshot);
                    return;
                }
                if (response.frontendChecked() != baseline.frontendChecked()
                        || response.frontendAccepted() != baseline.frontendAccepted()
                        || !response.diagnostics().equals(baseline.diagnostics())) {
                    // Identity checks passed, so repeated disagreement is a
                    // protocol failure, not permission to retry indefinitely.
                    throw new IOException("Nagi query and queryless source facts disagree for the same editor snapshot.");
                }
                if (baseline.hasSymbolIndex()) response = response.withSymbolIndex(baseline);
            }
            cancellation.checkDeadline();
            Snapshot responseSnapshot = withCompilerSources(withCompilerDirectories(snapshot, response), response, attempt);
            cancellation.checkDeadline();
            if (!snapshotCurrent(responseSnapshot)) {
                setStateForActive(key, State.STALE, "A Nagi source changed during compiler analysis; the response was discarded.");
                restartOpenSourcesLater();
                return;
            }
            CachedResponse result = new CachedResponse(responseSnapshot, response);
            List<DocumentationTicket> readyTickets;
            synchronized (lock) {
                if (!key.equals(activeKey) || responseSnapshot.epoch() != epoch.get()
                        || disposed || project.isDisposed()) return;
                cancellation.checkDeadline();
                cached = result;
                presentation.published(result.presentationIdentity, responseSnapshot.focusDocument());
                knownSourcePaths = knownPaths(snapshot, response);
                setState(State.READY, "Compiler assistance is current for this editor snapshot.");
                readyTickets = detachDocumentation(ticket -> ticket.owner() == attempt && ticket.epoch() == responseSnapshot.epoch()
                        && ticket.document() == responseSnapshot.focusDocument()
                        && ticket.stamp() == responseSnapshot.focusDocumentStamp());
            }
            notifyDocumentation(readyTickets, true);
            publish(result, currentWaiters(key));
        } catch (InterruptedException exception) {
            Thread.currentThread().interrupt();
            setStateForActive(key, State.STALE, "The previous Nagi analysis was cancelled.");
        } catch (com.intellij.openapi.progress.ProcessCanceledException cancelled) {
            setStateForActive(key, State.STALE, "The Nagi source snapshot was cancelled by a new editor write.");
            restartOpenSourcesLater();
        } catch (SnapshotBlockedException exception) {
            continuation = exception.documentationRecapture;
            if (exception.compilerUnavailable != null) {
                setCompilerUnavailableForActive(key, exception.compilerUnavailable, exception.getMessage());
            } else {
                setStateForActive(key, exception.state, exception.getMessage());
            }
        } catch (Exception exception) {
            setStateForActive(key, State.FAILED, safeMessage(exception));
        } catch (AssertionError failure) {
            setStateForActive(key, State.FAILED, safeMessage(failure));
            com.intellij.openapi.diagnostic.Logger.getInstance(NagiAssistService.class).warn("Nagi editor assistance failed", failure);
        } finally {
            List<DocumentationTicket> abandoned = List.of();
            synchronized (lock) {
                if (key.equals(activeKey) && activeAttempt == attempt) {
                    if (state == State.WAITING || state == State.RUNNING) {
                        setState(State.STALE, "Compiler assistance ended without a current result.");
                    }
                    activeKey = null;
                    activeAttempt = null;
                    activeFuture = null;
                    activeCancellation = null;
                    activeWaiters = List.of();
                    if (continuation != null && continuation.attempt() == attempt
                            && intent.epoch() == epoch.get() && intent.document().getModificationStamp() == intent.documentStamp()
                            && !disposed && !project.isDisposed()) {
                        DocumentationRecapture transfer = continuation;
                        documentationTickets.replaceAll(ticket -> ticket.owner() == attempt ? ticket.ownedBy(transfer) : ticket);
                    } else {
                        abandoned = detachDocumentation(ticket -> ticket.owner() == attempt);
                    }
                }
            }
            notifyDocumentation(abandoned, false);
        }
    }

    private Snapshot buildSnapshot(Intent intent, String requestKey) throws SnapshotBlockedException {
        assertWorkerIo();
        RawCapture raw = NagiPlatform.read(() -> captureRaw(intent));
        if (raw == null) throw new SnapshotBlockedException(State.STALE, "The Nagi source changed before its snapshot was captured.");

        Path source = canonicalExisting(Path.of(raw.activePath()), false);
        if (!isNagiSource(source)) throw new SnapshotBlockedException(State.STALE, "Nagi assistance requires an existing .nagi or .low file.");
        Path manifest = NagiCommandPlan.findManifest(source);
        if (manifest != null) manifest = canonicalExisting(manifest, false);
        Path sourceLexical = Path.of(raw.activePath()).toAbsolutePath().normalize();
        for (RawFile rawFile : raw.files()) {
            if (!rawFile.unsaved() || !isManifest(rawFile.file())) continue;
            Path openManifest = Path.of(rawFile.path()).toAbsolutePath().normalize();
            Path parent = openManifest.getParent();
            if (parent == null || !sourceLexical.startsWith(parent)) continue;
            boolean isSelected = manifest == null || samePath(openManifest.toString(), manifest.toString());
            if (!isSelected && Files.exists(openManifest)) {
                try { isSelected = samePath(openManifest.toRealPath().toString(), manifest.toString()); }
                catch (IOException ignored) { }
            }
            if (isSelected) throw new SnapshotBlockedException(State.UNSAVED_MANIFEST,
                    "Save nagi.toml before requesting compiler assistance; the IDE does not save it automatically.");
        }
        if (raw.exceedsOverlayLimit()) throw new SnapshotBlockedException(State.LIMIT, "Unsaved Nagi buffers exceed the 8 MB editor snapshot limit.");
        if (raw.exceedsTrackedFileLimit() || raw.files().size() > MAX_TRACKED_FILES) {
            throw new SnapshotBlockedException(State.LIMIT, "Too many open Nagi source files to snapshot safely.");
        }
        registerDocumentListeners(raw.files());
        Path workspace = raw.projectBase() == null || raw.projectBase().isBlank()
                ? (manifest == null ? source.getParent() : manifest.getParent())
                : canonicalExisting(Path.of(raw.projectBase()), true);

        Map<String, String> canonicalPaths = new HashMap<>();
        List<NagiAssistProtocol.Overlay> overlays = new ArrayList<>();
        List<DocumentStamp> documentStamps = new ArrayList<>();
        List<FileStamp> fileStamps = new ArrayList<>();
        List<VirtualFile> openSources = new ArrayList<>();
        for (RawFile rawFile : raw.files()) {
            documentStamps.add(new DocumentStamp(rawFile.document(), rawFile.documentStamp()));
            fileStamps.add(new FileStamp(rawFile.file(), rawFile.fileStamp()));
            if (!isTrackedSource(rawFile.file())) continue;
            openSources.add(rawFile.file());
            Path actual = canonicalExisting(Path.of(rawFile.path()), false);
            canonicalPaths.put(normalizePath(rawFile.path()), actual.toString());
            if (rawFile.unsaved()) overlays.add(new NagiAssistProtocol.Overlay(actual.toString(), rawFile.text()));
        }
        Path focusPath = canonicalExisting(Path.of(raw.activePath()), false);
        canonicalPaths.put(normalizePath(raw.activePath()), focusPath.toString());
        documentStamps.add(new DocumentStamp(raw.focusDocument(), raw.focusStamp()));
        fileStamps.add(new FileStamp(raw.focusFile(), raw.focusFileStamp()));
        boolean focusUnsaved = raw.files().stream().anyMatch(file -> file.document() == raw.focusDocument() && file.unsaved());
        if (focusUnsaved
                && overlays.stream().noneMatch(overlay -> samePath(overlay.file(), focusPath.toString()))) {
            String text = NagiPlatform.read(raw.focusDocument()::getText);
            overlays.add(new NagiAssistProtocol.Overlay(focusPath.toString(), text));
        }

        if (manifest != null) {
            for (RawFile rawFile : raw.files()) {
                if (isManifest(rawFile.file()) && rawFile.unsaved()
                        && samePath(Path.of(rawFile.path()).toAbsolutePath().normalize().toString(), manifest.toString())) {
                    throw new SnapshotBlockedException(State.UNSAVED_MANIFEST,
                            "Save nagi.toml before requesting compiler assistance; the IDE does not save it automatically.");
                }
            }
            VirtualFile manifestFile = LocalFileSystem.getInstance().findFileByIoFile(manifest.toFile());
            ManifestState manifestState = manifestFile == null ? null : NagiPlatform.read(() -> {
                Document document = FileDocumentManager.getInstance().getDocument(manifestFile);
                return document == null ? null : new ManifestState(document,
                        FileDocumentManager.getInstance().isDocumentUnsaved(document), document.getModificationStamp(), manifestFile.getModificationStamp());
            });
            if (manifestState != null) {
                if (manifestState.unsaved()) throw new SnapshotBlockedException(State.UNSAVED_MANIFEST,
                        "Save nagi.toml before requesting compiler assistance; the IDE does not save it automatically.");
                documentStamps.add(new DocumentStamp(manifestState.document(), manifestState.documentStamp()));
                fileStamps.add(new FileStamp(manifestFile, manifestState.fileStamp()));
            }
        }
        overlays.sort(Comparator.comparing(NagiAssistProtocol.Overlay::file));
        NagiAssistProtocol.Query query = raw.position() == null ? null
                : new NagiAssistProtocol.Query(focusPath.toString(), raw.position().line(), raw.position().column());
        byte[] input;
        try { input = NagiAssistProtocol.encode(new NagiAssistProtocol.Request(overlays, query)); }
        catch (IllegalArgumentException exception) {
            throw new SnapshotBlockedException(State.LIMIT, exception.getMessage());
        }

        Path executable;
        ExecutableStamp executableStamp;
        try {
            executable = compilerResolver.resolve(raw.compilerSetting(), workspace);
            VirtualFile compilerFile = LocalFileSystem.getInstance().findFileByPath(executable.toString());
            BasicFileAttributes attributes = Files.readAttributes(executable, BasicFileAttributes.class);
            executableStamp = new ExecutableStamp(compilerFile,
                    compilerFile == null ? -1 : compilerFile.getModificationStamp(), attributes.size(),
                    attributes.lastModifiedTime().toMillis(), String.valueOf(attributes.fileKey()));
        } catch (IOException | RuntimeException exception) {
            var unavailable = new CompilerUnavailable(
                    new CompilerSelection(raw.compilerSetting(), raw.projectBase() == null ? "" : raw.projectBase()),
                    compilerSearchPaths(raw.compilerSetting(), workspace));
            throw new SnapshotBlockedException(State.COMPILER_UNAVAILABLE, COMPILER_UNAVAILABLE_MESSAGE, unavailable);
        }
        if (manifest != null) {
            VirtualFile manifestFile = LocalFileSystem.getInstance().findFileByIoFile(manifest.toFile());
            if (manifestFile != null) fileStamps.add(new FileStamp(manifestFile, manifestFile.getModificationStamp()));
        }
        fileStamps.add(new FileStamp(raw.focusFile(), raw.focusFileStamp()));
        LinkedHashSet<Path> diskPaths = new LinkedHashSet<>();
        diskPaths.add(focusPath);
        diskPaths.add(manifest == null ? source : manifest);
        for (String dependency : knownDependencies) {
            Path dependencyPath = Path.of(NagiAssistProtocol.physicalPath(dependency));
            // Removed imports are rediscovered from the new compiler graph.
            if (Files.isRegularFile(dependencyPath)) diskPaths.add(dependencyPath);
        }
        List<DiskStamp> diskStamps = readDiskSnapshot(diskPaths);
        List<DirectoryStamp> directoryStamps = new ArrayList<>();
        if (query != null && samePath(query.file(), importQueryFile)) {
            for (String directory : knownImportDirectories) directoryStamps.add(readDirectorySnapshot(Path.of(NagiAssistProtocol.physicalPath(directory))));
        }
        NagiAssistCommandPlan plan = NagiAssistCommandPlan.create(executable.toString(), source, manifest, workspace, SystemInfo.isWindows);
        return new Snapshot(raw.epoch(), requestKey, raw.compilerSetting(), raw.timeout(), raw.projectBase(),
                manifest == null ? source : manifest, focusPath, executable, executableStamp, plan, query, input,
                distinctDocuments(documentStamps), distinctFiles(fileStamps), diskStamps, List.copyOf(directoryStamps), Map.copyOf(canonicalPaths),
                List.copyOf(new LinkedHashSet<>(openSources)), raw.focusDocument(), raw.focusStamp(),
                raw.focusFile(), raw.focusFileStamp(), raw.editor(), raw.position() == null ? -1 : raw.position().offset());
    }

    private Snapshot withCompilerDirectories(Snapshot snapshot, NagiAssistProtocol.Response response) throws SnapshotBlockedException {
        assertWorkerIo();
        NagiAssistProtocol.ImportContext context = response.importContext();
        if (context != null && (snapshot.query() == null || !samePath(context.replacement().file(), snapshot.query().file())))
            throw new SnapshotBlockedException(State.FAILED, "Import completion does not belong to this query file.");
        Set<String> directories = context == null ? Set.of() : Set.copyOf(context.directories());
        knownImportDirectories = directories;
        importQueryFile = context == null ? null : context.replacement().file();
        if (!updateImportDirectoryWatch(directories, snapshot)) {
            throw new SnapshotBlockedException(State.STALE,
                    "The import directory is being observed asynchronously; its candidates were discarded.");
        }
        if (disposed || project.isDisposed()) {
            knownImportDirectories = Set.of();
            importQueryFile = null;
            throw new SnapshotBlockedException(State.CLOSED, "Project is closed.");
        }
        if (directories.stream().anyMatch(path -> snapshot.directories().stream().noneMatch(stamp -> samePath(path, stamp.path().toString())))) {
            retrySnapshotLater(snapshot);
            throw new SnapshotBlockedException(State.STALE, "New import directories require a fresh snapshot; the response was discarded.");
        }
        // A later expression response must not retain the former import query's directory dependency.
        if (!directories.isEmpty()) return snapshot;
        return new Snapshot(snapshot.epoch(), snapshot.requestKey(), snapshot.compilerSetting(), snapshot.timeout(),
                snapshot.projectBase(), snapshot.scopePath(), snapshot.focusPath(), snapshot.executable(), snapshot.executableStamp(),
                snapshot.plan(), snapshot.query(), snapshot.input(), snapshot.documents(), snapshot.files(), snapshot.disks(), List.of(),
                snapshot.canonicalPaths(), snapshot.openSources(), snapshot.focusDocument(), snapshot.focusDocumentStamp(),
                snapshot.focusFile(), snapshot.focusFileStamp(), snapshot.editor(), snapshot.caretOffset());
    }

    private Snapshot withCompilerSources(Snapshot snapshot, NagiAssistProtocol.Response response, Object attempt) throws SnapshotBlockedException {
        assertWorkerIo();
        List<FileStamp> stamps = new ArrayList<>(snapshot.files());
        Map<String, String> canonicalPaths = new HashMap<>(snapshot.canonicalPaths());
        LinkedHashSet<String> seen = new LinkedHashSet<>();
        LinkedHashSet<String> dependencies = new LinkedHashSet<>();
        for (String source : response.sourceFiles()) if (!source.startsWith("stdlib:")) dependencies.add(source);
        for (NagiAssistProtocol.Diagnostic diagnostic : response.diagnostics()) {
            if (diagnostic.file() != null && !diagnostic.file().isBlank() && !diagnostic.file().startsWith("stdlib:")
                    && Files.isRegularFile(Path.of(NagiAssistProtocol.physicalPath(diagnostic.file()))))
                dependencies.add(diagnostic.file());
        }
        knownDependencies = Set.copyOf(dependencies);
        boolean discovered = dependencies.stream().anyMatch(source -> snapshot.disks().stream()
                .noneMatch(stamp -> samePath(stamp.path().toString(), source)));
        if (discovered) {
            // A graph first reported after analysis was not captured before it.
            // Never associate those facts with post-hoc dependency stamps.
            var transfer = new DocumentationRecapture(attempt, snapshot.focusFile(), snapshot.focusDocument(),
                    snapshot.focusDocumentStamp(), snapshot.epoch(),
                    documentationGroup(snapshot.focusFile(), snapshot.focusDocumentStamp(), snapshot.epoch()));
            Editor recaptureEditor = snapshot.editor();
            if (enqueue(() -> ApplicationManager.getApplication().invokeLater(() -> resumeDocumentationRecapture(transfer, recaptureEditor)))) {
                throw new SnapshotBlockedException(State.STALE,
                        "New compiler dependencies require a fresh disk snapshot; the response was discarded.", null, transfer);
            }
            throw new SnapshotBlockedException(State.STALE, "The dependency snapshot could not be rescheduled.");
        }
        for (String source : response.sourceFiles()) addCompilerSource(snapshot, stamps, canonicalPaths, seen, source);
        for (NagiAssistProtocol.Diagnostic diagnostic : response.diagnostics()) {
            if (diagnostic.file() != null && !diagnostic.file().isBlank()
                    && !diagnostic.file().startsWith("stdlib:") && Files.isRegularFile(Path.of(NagiAssistProtocol.physicalPath(diagnostic.file())))) {
                addCompilerSource(snapshot, stamps, canonicalPaths, seen, diagnostic.file());
            }
        }
        return new Snapshot(snapshot.epoch(), snapshot.requestKey(), snapshot.compilerSetting(), snapshot.timeout(),
                snapshot.projectBase(), snapshot.scopePath(), snapshot.focusPath(), snapshot.executable(),
                snapshot.executableStamp(), snapshot.plan(), snapshot.query(), snapshot.input(), snapshot.documents(),
                distinctFiles(stamps), snapshot.disks(), snapshot.directories(), Map.copyOf(canonicalPaths), snapshot.openSources(), snapshot.focusDocument(),
                snapshot.focusDocumentStamp(), snapshot.focusFile(), snapshot.focusFileStamp(), snapshot.editor(), snapshot.caretOffset());
    }

    private void addCompilerSource(Snapshot snapshot, List<FileStamp> stamps, Map<String, String> canonicalPaths,
                                          Set<String> seen, String source) throws SnapshotBlockedException {
        if (source.startsWith("stdlib:")) return;
        Path actual;
        try { actual = canonicalExisting(Path.of(NagiAssistProtocol.physicalPath(source)), false); }
        catch (InvalidPathException exception) {
            throw new SnapshotBlockedException(State.STALE, "The compiler returned an invalid source path; its response was discarded.");
        }
        String canonical = actual.toString();
        if (!seen.add(normalizePath(canonical))) return;
        VirtualFile file = LocalFileSystem.getInstance().findFileByPath(canonical);
        if (file == null) {
            // A synchronous refresh from this worker can wait for an EDT write
            // while editor readers wait for assistance. Discover just the
            // missing source asynchronously, discard this response, and then
            // capture a new complete snapshot rather than caching unstamped facts.
            ApplicationManager.getApplication().invokeLater(() -> {
                if (disposed || project.isDisposed()) return;
                LocalFileSystem.getInstance().refreshNioFiles(List.of(actual), true, false,
                        () -> retrySnapshotLater(snapshot));
            });
            throw new SnapshotBlockedException(State.STALE, "An imported Nagi source is being discovered asynchronously; its response was discarded.");
        }
        if (file == null || !file.isValid() || !file.isInLocalFileSystem()) {
            throw new SnapshotBlockedException(State.STALE, "A compiler source file changed while assistance was running.");
        }
        canonicalPaths.put(normalizePath(source), canonical);
        stamps.add(new FileStamp(file, file.getModificationStamp()));
    }

    private static String documentationGroup(VirtualFile file, long stamp, long generation) {
        return normalizePath(file.getPath()) + ":" + stamp + ":" + generation + ":";
    }

    /** Existing dependency recapture, with an explicit bounded ticket responsibility transfer. */
    private void resumeDocumentationRecapture(DocumentationRecapture transfer, Editor recaptureEditor) {
        Admission admission;
        try {
            admission = NagiPlatform.read(() -> {
                if (!transfer.file().isValid()) return Admission.REJECTED;
                PsiFile file = PsiManager.getInstance(project).findFile(transfer.file());
                if (file == null || !documentationIdentityCurrent(file, transfer.document(),
                        transfer.stamp(), transfer.epoch())) return Admission.REJECTED;
                if (fresh(file) != null) return Admission.CURRENT;
                return recaptureEditor == null ? documentationAdmission(file)
                        : request(file, recaptureEditor, null, DIAGNOSTIC_DEBOUNCE_MS);
            });
        } catch (RuntimeException | Error failure) {
            List<DocumentationTicket> abandoned;
            synchronized (lock) { abandoned = detachDocumentation(ticket -> ticket.owner() == transfer); }
            notifyDocumentation(abandoned, false);
            throw failure;
        }
        List<DocumentationTicket> detached = List.of();
        synchronized (lock) {
            if (admission == Admission.CURRENT || admission == Admission.REJECTED) {
                detached = detachDocumentation(ticket -> ticket.owner() == transfer);
            } else {
                Object owner = activeKey != null && activeKey.startsWith(transfer.group()) ? activeAttempt : null;
                documentationTickets.replaceAll(ticket -> ticket.owner() == transfer ? ticket.ownedBy(owner) : ticket);
            }
        }
        notifyDocumentation(detached, admission == Admission.CURRENT);
    }

    private void retrySnapshotLater(Snapshot snapshot) {
        // This serial worker task necessarily follows execute's finally. A
        // direct EDT callback could race activeKey cleanup and lose the retry.
        enqueue(() -> ApplicationManager.getApplication().invokeLater(() -> {
            if (disposed || project.isDisposed() || !snapshot.focusFile().isValid()) return;
            PsiFile focus = NagiPlatform.read(() -> PsiManager.getInstance(project).findFile(snapshot.focusFile()));
            if (focus != null) requestBaseline(focus, snapshot.editor());
        }));
    }

    private static List<DiskStamp> readDiskSnapshot(Set<Path> paths) throws SnapshotBlockedException {
        assertWorkerIo();
        if (paths.size() > NagiAssistProtocol.MAX_FILES + 1)
            throw new SnapshotBlockedException(State.LIMIT, "Too many dependency files for a bounded editor snapshot.");
        List<DiskStamp> stamps = new ArrayList<>();
        long bytes = 0;
        for (Path path : paths) {
            try (var input = Files.newInputStream(path)) {
                byte[] contents = input.readNBytes(NagiAssistProtocol.MAX_OVERLAY_BYTES + 1);
                bytes += contents.length;
                if (bytes > NagiAssistProtocol.MAX_OVERLAY_BYTES)
                    throw new SnapshotBlockedException(State.LIMIT, "Dependency disk snapshot exceeds 8 MB.");
                stamps.add(new DiskStamp(path, contents));
            } catch (IOException exception) {
                throw new SnapshotBlockedException(State.STALE, "A compiler input changed while its disk snapshot was captured.");
            }
        }
        return List.copyOf(stamps);
    }

    /** Worker-only, bounded filename/kind observation of the exact compiler-reported parent. */
    private static DirectoryStamp readDirectorySnapshot(Path path) throws SnapshotBlockedException {
        assertWorkerIo();
        List<DirectoryEntry> entries = new ArrayList<>();
        try {
            BasicFileAttributes before = Files.readAttributes(path, BasicFileAttributes.class);
            boolean incomplete = false;
            try (var stream = Files.newDirectoryStream(path)) {
                for (Path entry : stream) {
                    if (Thread.currentThread().isInterrupted()) throw new SnapshotBlockedException(State.STALE, "Import directory capture was cancelled.");
                    if (entries.size() == MAX_DIRECTORY_ENTRIES) { incomplete = true; entries.clear(); break; }
                    BasicFileAttributes attributes = Files.readAttributes(entry, BasicFileAttributes.class, java.nio.file.LinkOption.NOFOLLOW_LINKS);
                    String kind = attributes.isSymbolicLink() ? "link:" + Files.readSymbolicLink(entry)
                            : attributes.isDirectory() ? "directory" : attributes.isRegularFile() ? "file" : "other";
                    entries.add(new DirectoryEntry(entry.getFileName().toString(), kind));
                }
            }
            BasicFileAttributes after = Files.readAttributes(path, BasicFileAttributes.class);
            if (!before.lastModifiedTime().equals(after.lastModifiedTime()) || !Objects.equals(before.fileKey(), after.fileKey()))
                throw new SnapshotBlockedException(State.STALE, "Import directory changed during capture.");
            entries.sort(Comparator.comparing(DirectoryEntry::name).thenComparing(DirectoryEntry::kind));
            return new DirectoryStamp(path, List.copyOf(entries), incomplete, after.lastModifiedTime(), String.valueOf(after.fileKey()));
        } catch (IOException | java.nio.file.DirectoryIteratorException exception) {
            throw new SnapshotBlockedException(State.STALE, "Import directory changed during capture.");
        }
    }

    private static void assertWorkerIo() {
        com.intellij.util.concurrency.ThreadingAssertions.assertBackgroundThread();
    }

    private boolean updateImportDirectoryWatch(Set<String> directories, Snapshot snapshot) throws SnapshotBlockedException {
        assertWorkerIo();
        String next = directories.isEmpty() ? null : directories.iterator().next();
        if (disposed || project.isDisposed()) next = null;
        if (Objects.equals(next, watchedImportDirectory)
                && (next == null || watchedImportVirtualDirectory != null && watchedImportVirtualDirectory.isValid()))
            return next == null || importDirectoryPrimed;
        clearImportDirectoryWatch();
        if (next == null) return true;
        LocalFileSystem.WatchRequest watch = LocalFileSystem.getInstance()
                .addRootToWatch(NagiAssistProtocol.physicalPath(next), false);
        if (watch == null) throw new SnapshotBlockedException(State.FAILED,
                "The import directory could not be watched; its candidates were discarded.");
        importDirectoryWatch = watch;
        watchedImportDirectory = next;
        try {
            // Watching a partially loaded VFS directory only dirties its cached
            // children: arbitrary future sibling names would remain invisible.
            // Prime this exact parent once, on the worker, after our bounded scan.
            DirectoryStamp before = readDirectorySnapshot(Path.of(NagiAssistProtocol.physicalPath(next)));
            if (before.incomplete()) throw new SnapshotBlockedException(State.LIMIT,
                    "The import directory exceeds the bounded editor catalog.");
            VirtualFile directory = LocalFileSystem.getInstance().findFileByPath(NagiAssistProtocol.physicalPath(next));
            if (directory == null || !directory.isValid() || !directory.isDirectory())
                throw new SnapshotBlockedException(State.STALE, "The import directory changed before it could be observed.");
            watchedImportVirtualDirectory = directory;
            // The platform's child loading has no bounded variant. It is neither
            // recursive nor run under a caller-held read action or on the EDT.
            VfsUtil.markDirty(false, true, directory);
            DirectoryStamp after = readDirectorySnapshot(before.path());
            if (after.incomplete()) throw new SnapshotBlockedException(State.LIMIT,
                    "The import directory exceeds the bounded editor catalog.");
            String observed = next;
            directory.refresh(true, false, () -> enqueue(() -> {
                if (disposed || project.isDisposed() || !Objects.equals(observed, watchedImportDirectory)
                        || directory != watchedImportVirtualDirectory || !directory.isValid()) return;
                importDirectoryPrimed = true;
                retrySnapshotLater(snapshot);
            }));
        } catch (SnapshotBlockedException | RuntimeException exception) {
            // A cancelled priming scan must not leave a watch permanently
            // waiting for a refresh callback that was never scheduled.
            clearImportDirectoryWatch();
            throw exception;
        }
        // Disposal can race platform watch registration. Cleanup remains on
        // this worker; no UI callback waits for a filesystem operation.
        if (disposed || project.isDisposed()) clearImportDirectoryWatch();
        return false;
    }

    private void clearImportDirectoryWatch() {
        assertWorkerIo();
        LocalFileSystem.WatchRequest watch = importDirectoryWatch;
        importDirectoryWatch = null;
        watchedImportDirectory = null;
        watchedImportVirtualDirectory = null;
        importDirectoryPrimed = false;
        if (watch != null) LocalFileSystem.getInstance().removeWatchedRoot(watch);
    }

    /** Worker-only content comparison, including closed imports and arbitrary native file extensions. */
    private static boolean diskSnapshotCurrent(Snapshot snapshot) {
        assertWorkerIo();
        for (DirectoryStamp stamp : snapshot.directories()) {
            try {
                DirectoryStamp current = readDirectorySnapshot(stamp.path());
                if (current.incomplete() != stamp.incomplete() || !current.entries().equals(stamp.entries())
                        || !current.modified().equals(stamp.modified()) || !current.fileKey().equals(stamp.fileKey())) return false;
            } catch (SnapshotBlockedException exception) { return false; }
        }
        for (DiskStamp stamp : snapshot.disks()) {
            try (var input = Files.newInputStream(stamp.path())) {
                byte[] contents = input.readNBytes(stamp.contents().length + 1);
                if (!java.util.Arrays.equals(contents, stamp.contents())) return false;
            } catch (IOException exception) { return false; }
        }
        return true;
    }

    private RawCapture captureRaw(Intent intent) {
        if (project.isDisposed() || !intent.file().isValid() || intent.document().getModificationStamp() != intent.documentStamp()) return null;
        FileDocumentManager documentManager = FileDocumentManager.getInstance();
        List<RawFile> files = new ArrayList<>();
        long overlayCharacters = 0;
        boolean exceedsOverlayLimit = false;
        boolean exceedsTrackedFileLimit = false;
        List<VirtualFile> openFiles = new ArrayList<>(java.util.Arrays.asList(FileEditorManager.getInstance(project).getOpenFiles()));
        openFiles.sort(Comparator.comparing((VirtualFile file) -> !isManifest(file)).thenComparing(VirtualFile::getPath));
        for (VirtualFile file : openFiles) {
            if (!isTrackedSource(file) && !isManifest(file)) continue;
            Document document = documentManager.getDocument(file);
            if (document == null) continue;
            boolean unsaved = documentManager.isDocumentUnsaved(document);
            if (files.size() >= MAX_TRACKED_FILES) {
                exceedsTrackedFileLimit = true;
                if (isManifest(file) && unsaved) {
                    files.add(new RawFile(file, document, file.getPath(), document.getModificationStamp(),
                            file.getModificationStamp(), true, null));
                }
                continue;
            }
            String text = null;
            if (unsaved && isTrackedSource(file)) {
                overlayCharacters += document.getTextLength();
                if (overlayCharacters > NagiAssistProtocol.MAX_OVERLAY_BYTES) {
                    exceedsOverlayLimit = true;
                } else {
                    text = document.getText();
                }
            }
            files.add(new RawFile(file, document, file.getPath(), document.getModificationStamp(),
                    file.getModificationStamp(), unsaved, text));
        }
        if (files.stream().noneMatch(file -> file.document() == intent.document())) {
            boolean unsaved = documentManager.isDocumentUnsaved(intent.document());
            if (unsaved && isTrackedSource(intent.file())) {
                overlayCharacters += intent.document().getTextLength();
                if (overlayCharacters > NagiAssistProtocol.MAX_OVERLAY_BYTES) exceedsOverlayLimit = true;
            }
            if (files.size() >= MAX_TRACKED_FILES) exceedsTrackedFileLimit = true;
            files.add(new RawFile(intent.file(), intent.document(), intent.file().getPath(), intent.document().getModificationStamp(),
                    intent.file().getModificationStamp(), unsaved,
                    unsaved && !exceedsOverlayLimit ? intent.document().getText() : null));
        }
        NagiSettings.Values settings = NagiSettings.getInstance().getState();
        return new RawCapture(intent.file(), intent.document(), intent.document().getModificationStamp(),
                intent.file().getModificationStamp(), List.copyOf(files), project.getBasePath(),
                settings.compilerPath == null ? "" : settings.compilerPath, settings.checkTimeoutSeconds,
                intent.epoch(), intent.position(), intent.editor(), intent.file().getPath(), exceedsOverlayLimit,
                exceedsTrackedFileLimit);
    }

    private boolean snapshotCurrent(Snapshot snapshot) {
        if (disposed || project.isDisposed() || snapshot.epoch != epoch.get()) return false;
        if (!trustedForCompiler()) return false;
        NagiSettings.Values settings = NagiSettings.getInstance().getState();
        if (!Objects.equals(settings.compilerPath == null ? "" : settings.compilerPath, snapshot.compilerSetting)
                || settings.checkTimeoutSeconds != snapshot.timeout
                || !Objects.equals(project.getBasePath(), snapshot.projectBase)) return false;
        try {
            boolean current = NagiPlatform.read(() -> {
                for (DocumentStamp stamp : snapshot.documents) {
                    if (stamp.document().getModificationStamp() != stamp.stamp()) return false;
                }
                for (FileStamp stamp : snapshot.files) {
                    if (!stamp.file().isValid() || stamp.file().getModificationStamp() != stamp.stamp()) return false;
                }
                return true;
            });
            if (!current) return false;
            return snapshot.executableStamp.file() == null || (snapshot.executableStamp.file().isValid()
                    && snapshot.executableStamp.file().getModificationStamp() == snapshot.executableStamp.virtualStamp());
        } catch (RuntimeException exception) {
            return false;
        }
    }

    private static boolean executableIdentityCurrent(Snapshot snapshot) {
        assertWorkerIo();
        try {
            if (snapshot.executableStamp.file() != null && (!snapshot.executableStamp.file().isValid()
                    || snapshot.executableStamp.file().getModificationStamp() != snapshot.executableStamp.virtualStamp())) return false;
            BasicFileAttributes attributes = Files.readAttributes(snapshot.executable, BasicFileAttributes.class);
            return attributes.size() == snapshot.executableStamp.size()
                    && attributes.lastModifiedTime().toMillis() == snapshot.executableStamp.modifiedMillis()
                    && Objects.equals(String.valueOf(attributes.fileKey()), snapshot.executableStamp.fileKey());
        } catch (IOException | RuntimeException exception) { return false; }
    }

    private void publish(CachedResponse response, List<PopupWaiter> waiters) {
        ApplicationManager.getApplication().invokeLater(() -> {
            if (disposed || project.isDisposed() || response != cached || !snapshotCurrent(response.snapshot)) return;
            for (VirtualFile source : response.snapshot.openSources) {
                if (!source.isValid()) continue;
                PsiFile psi = NagiPlatform.read(() -> PsiManager.getInstance(project).findFile(source));
                if (psi != null) NagiPlatform.restart(psi);
            }
            for (PopupWaiter waiter : waiters) {
                if (!isPopupFresh(response, waiter)) continue;
                if (waiter.kind() == PopupKind.PARAMETERS && waiter.respectParameterSetting()
                        && !CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO) continue;
                String key = response.snapshot.requestKey + ":" + waiter.kind();
                synchronized (lock) {
                    if (popupScheduled.contains(key)) continue;
                    popupScheduled.add(key);
                    while (popupScheduled.size() > 32) popupScheduled.remove(popupScheduled.iterator().next());
                }
                if (waiter.kind() == PopupKind.PARAMETERS) {
                    VirtualFile[] selected = FileEditorManager.getInstance(project).getSelectedFiles();
                    if (selected.length > 0 && java.util.Arrays.stream(selected).noneMatch(file -> file == waiter.file())) continue;
                    PsiFile file = PsiManager.getInstance(project).findFile(waiter.file());
                    if (file != null && NagiParameterInfoHandler.callAt(file, waiter.editor(), (int) waiter.offset()) != null)
                        new ShowParameterInfoHandler().invoke(project, waiter.editor(), file);
                } else AutoPopupController.getInstance(project).scheduleAutoPopup(waiter.editor());
            }
        });
    }

    private boolean isPopupFresh(CachedResponse response, PopupWaiter waiter) {
        if (response != cached || waiter.editor().isDisposed() || !waiter.file().isValid()
                || !snapshotCurrent(response.snapshot)) return false;
        if (waiter.document().getModificationStamp() != waiter.stamp()
                || waiter.editor().getDocument() != waiter.document()
                || waiter.editor().getCaretModel().getOffset() != waiter.offset()) return false;
        Position current = position(waiter.editor());
        return current != null && response.snapshot.query != null
                && current.line() == response.snapshot.query.line()
                && current.column() == response.snapshot.query.column()
                && current.offset() == waiter.offset()
                && samePath(response.snapshot.query.file(), canonicalPathFor(response.snapshot, waiter.file()));
    }

    private void restartOpenSourcesLater() {
        ApplicationManager.getApplication().invokeLater(this::restartOpenSources);
    }

    private boolean enqueue(Runnable task) {
        try { worker.execute(task); return true; }
        catch (java.util.concurrent.RejectedExecutionException exception) {
            if (!disposed) setState(State.CLOSED, "Project is closed.");
            return false;
        }
    }

    private void restartOpenSources() {
        if (disposed || project.isDisposed()) return;
        var sources = new LinkedHashSet<VirtualFile>(java.util.Arrays.asList(FileEditorManager.getInstance(project).getOpenFiles()));
        if (lastFocusFile != null) sources.add(lastFocusFile);
        for (VirtualFile file : sources) {
            if (!isTrackedSource(file)) continue;
            PsiFile psi = NagiPlatform.read(() -> PsiManager.getInstance(project).findFile(file));
            if (psi != null) NagiPlatform.restart(psi);
        }
    }

    private void attachOpenDocuments() {
        if (disposed || project.isDisposed()) return;
        List<RawFile> open = NagiPlatform.read(() -> {
            List<RawFile> files = new ArrayList<>();
            FileDocumentManager manager = FileDocumentManager.getInstance();
            for (VirtualFile file : FileEditorManager.getInstance(project).getOpenFiles()) {
                if (!isTrackedSource(file) && !isManifest(file)) continue;
                Document document = manager.getDocument(file);
                if (document != null) files.add(new RawFile(file, document, file.getPath(), document.getModificationStamp(),
                        file.getModificationStamp(), manager.isDocumentUnsaved(document), null));
            }
            return files;
        });
        registerDocumentListeners(open);
    }

    private void registerDocumentListeners(List<RawFile> files) {
        synchronized (documentListeners) {
            // A worker may have captured these documents before disposal began.
            if (disposed || project.isDisposed()) return;
            for (RawFile raw : files) {
                if (documentListeners.containsKey(raw.document())) continue;
                DocumentListener listener = new DocumentListener() {
                    @Override public void beforeDocumentChange(@NotNull DocumentEvent event) {
                        presentation.beforeEdit(event);
                    }
                    @Override public void documentChanged(@NotNull DocumentEvent event) {
                        presentation.afterEdit(event);
                        invalidate(State.STALE, "An open Nagi source or manifest changed.",
                                !isManifest(raw.file()) && presentation.softEdit(event.getDocument()));
                    }
                };
                // Complete the document subscription before publishing its
                // token under the service: a closing parent must not dispose
                // the token before Document has attached its cleanup child.
                Disposable subscription = Disposer.newDisposable("Nagi document listener");
                boolean retained = false;
                try {
                    raw.document().addDocumentListener(listener, subscription);
                    if (!Disposer.tryRegister(this, subscription)) continue;
                    documentListeners.put(raw.document(), new ListenerRecord(raw.file(), subscription));
                    retained = true;
                } finally {
                    if (!retained) Disposer.dispose(subscription);
                }
            }
        }
    }

    private void detachDocument(VirtualFile file) {
        synchronized (documentListeners) {
            List<Document> remove = documentListeners.entrySet().stream()
                    .filter(entry -> entry.getValue().file() == file)
                    .map(Map.Entry::getKey).toList();
            for (Document document : remove) {
                ListenerRecord record = documentListeners.remove(document);
                Disposer.dispose(record.subscription());
            }
        }
    }

    private boolean isRelevantVfsEvent(VFileEvent event) {
        boolean directoryChanged = event.getFile() != null && event.getFile().isDirectory();
        return eventPaths(event).stream().anyMatch(path -> isRelevantVfsPath(path, directoryChanged));
    }

    /** Event paths are VFS memory only. After a rename/move, getPath alone can
     * miss removal from the previously watched parent or an ancestor rename. */
    private static Set<String> eventPaths(VFileEvent event) {
        LinkedHashSet<String> paths = new LinkedHashSet<>();
        paths.add(normalizePath(event.getPath()));
        if (event instanceof VFileMoveEvent move) {
            String name = move.getFile().getName();
            paths.add(normalizePath(move.getOldParent().getPath() + "/" + name));
            paths.add(normalizePath(move.getNewParent().getPath() + "/" + name));
        } else if (event instanceof VFilePropertyChangeEvent change
                && VirtualFile.PROP_NAME.equals(change.getPropertyName())) {
            VirtualFile parent = change.getFile().getParent();
            if (parent != null) {
                paths.add(normalizePath(parent.getPath() + "/" + change.getOldValue()));
                paths.add(normalizePath(parent.getPath() + "/" + change.getNewValue()));
            }
        }
        return Set.copyOf(paths);
    }

    private boolean isRelevantVfsPath(String path, boolean directoryChanged) {
        synchronized (lock) {
            if ((activeKey != null || pendingKey != null)
                    && (path.endsWith(".nagi") || path.endsWith(".low"))) return true;
        }
        if (knownSourcePaths.stream().anyMatch(known -> samePath(path, normalizePath(known)))) return true;
        if (knownImportDirectories.stream().map(NagiAssistService::normalizePath)
                .anyMatch(directory -> pathUnder(path, directory) || (directoryChanged && pathUnder(directory, path)))) return true;
        if (!(path.endsWith(".nagi") || path.endsWith(".low") || path.endsWith("/nagi.toml") || path.equals("nagi.toml"))) return false;
        String base = project.getBasePath();
        if (base != null && pathUnder(path, normalizePath(base))) return true;
        return knownSourcePaths.stream().filter(known -> !known.startsWith("stdlib:"))
                .map(known -> Path.of(NagiAssistProtocol.physicalPath(known)).getParent())
                .filter(Objects::nonNull)
                .anyMatch(parent -> pathUnder(path, normalizePath(parent.toString())));
    }

    private boolean isUnavailableCompilerVfsEvent(VFileEvent event) {
        return eventPaths(event).stream().anyMatch(unavailableCompilerPaths::contains);
    }

    private void clearUnavailableCompiler() {
        synchronized (lock) {
            unavailableCompilerSelection = null;
            presentedUnavailableSelection = null;
            unavailableCompilerPaths = Set.of();
        }
    }

    private CompilerSelection currentCompilerSelection() {
        NagiSettings.Values settings = NagiSettings.getInstance().getState();
        return new CompilerSelection(settings.compilerPath == null ? "" : settings.compilerPath,
                project.getBasePath() == null ? "" : project.getBasePath());
    }

    private boolean compilerUnavailableForCurrentSelection() {
        CompilerSelection unavailable = unavailableCompilerSelection;
        if (unavailable == null || !unavailable.equals(currentCompilerSelection())) return false;
        setState(State.COMPILER_UNAVAILABLE, COMPILER_UNAVAILABLE_MESSAGE);
        return true;
    }

    private void setCompilerUnavailableForActive(String key, CompilerUnavailable unavailable, String message) {
        boolean present;
        synchronized (lock) {
            if (!key.equals(activeKey)) return;
            unavailableCompilerSelection = unavailable.selection();
            unavailableCompilerPaths = unavailable.candidatePaths();
            setState(State.COMPILER_UNAVAILABLE, message);
            present = !unavailable.selection().equals(presentedUnavailableSelection);
            if (present) presentedUnavailableSelection = unavailable.selection();
        }
        if (!present) return;
        ApplicationManager.getApplication().invokeLater(() -> {
            if (disposed || project.isDisposed() || state != State.COMPILER_UNAVAILABLE
                    || !unavailable.selection().equals(unavailableCompilerSelection)) return;
            compilerUnavailablePresenter.show(project, () -> retryCompilerResolution(unavailable.selection()));
        });
    }

    private void retryCompilerResolution(CompilerSelection expected) {
        synchronized (lock) {
            if (disposed || !expected.equals(unavailableCompilerSelection)) return;
            unavailableCompilerSelection = null;
            presentedUnavailableSelection = null;
            unavailableCompilerPaths = Set.of();
            setState(State.STALE, "Retrying Nagi compiler assistance.");
        }
        restartOpenSourcesLater();
    }

    private static void showCompilerUnavailable(Project project, Runnable retry) {
        if (project.isDisposed()) return;
        Notification notification = NotificationGroupManager.getInstance()
                .getNotificationGroup(COMPILER_NOTIFICATION_GROUP)
                .createNotification("Nagi editor assistance is unavailable",
                        "Set the compiler executable in Settings | Tools | Nagi, or install nagic on PATH.",
                        NotificationType.INFORMATION);
        notification.addAction(NotificationAction.createSimple("Configure compiler", () ->
                ShowSettingsUtil.getInstance().showSettingsDialog(project, NagiConfigurable.class)));
        notification.addAction(NotificationAction.createSimple("Retry assistance", () -> {
            notification.expire();
            retry.run();
        }));
        Notifications.Bus.notify(notification, project);
    }

    private static Set<String> compilerSearchPaths(String configured, Path workspace) {
        LinkedHashSet<String> paths = new LinkedHashSet<>();
        String name = SystemInfo.isWindows ? "nagic.exe" : "nagic";
        try {
            if (configured != null && !configured.isBlank()) {
                Path candidate = Path.of(configured.strip());
                if (!candidate.isAbsolute()) candidate = workspace.resolve(candidate).normalize();
                paths.add(normalizePath(candidate.toAbsolutePath().normalize().toString()));
            } else {
                String path = System.getenv("PATH");
                if (path != null) {
                    for (String entry : path.split(java.util.regex.Pattern.quote(SystemInfo.isWindows ? ";" : ":"), -1)) {
                        Path directory = entry.isBlank() ? workspace : Path.of(entry);
                        paths.add(normalizePath(directory.resolve(name).toAbsolutePath().normalize().toString()));
                    }
                }
            }
        } catch (RuntimeException ignored) {
            // Invalid compiler settings remain a quiet configuration failure;
            // no path is watched until the setting is corrected.
        }
        return Set.copyOf(paths);
    }

    private void invalidate(State reason, String message) {
        invalidate(reason, message, false);
    }

    private void invalidate(State reason, String message, boolean keepRolePresentation) {
        if (!keepRolePresentation) presentation.clear();
        epoch.incrementAndGet();
        cached = null;
        List<DocumentationTicket> abandoned;
        synchronized (lock) {
            failedKey = null;
            if (pending != null) pending.cancel(false);
            pending = null;
            pendingKey = null;
            if (activeCancellation != null) activeCancellation.cancel();
            if (activeFuture != null) activeFuture.cancel(true);
            activeCancellation = null;
            activeFuture = null;
            activeKey = null;
            activeAttempt = null;
            pendingWaiters = List.of();
            activeWaiters = List.of();
            popupScheduled.clear();
            setState(reason, message);
            abandoned = detachDocumentation(ticket -> true);
        }
        notifyDocumentation(abandoned, false);
    }

    private void cancelPendingAndActiveLocked() {
        if (pending != null) pending.cancel(false);
        pending = null;
        pendingKey = null;
        if (activeCancellation != null) activeCancellation.cancel();
        if (activeFuture != null) activeFuture.cancel(true);
        activeCancellation = null;
        activeFuture = null;
        activeKey = null;
        activeAttempt = null;
        pendingWaiters = List.of();
        activeWaiters = List.of();
    }

    private void addPopupWaiter(Intent intent, Editor editor, PopupKind kind) {
        List<PopupWaiter> existing = intent.key().equals(activeKey) ? activeWaiters : pendingWaiters;
        if (existing.size() >= 8 || editor.isDisposed()) return;
        List<PopupWaiter> changed = new ArrayList<>(existing);
        PopupWaiter waiter = waiter(intent, editor, kind);
        if (changed.stream().noneMatch(candidate -> candidate.editor() == editor && candidate.kind() == kind)) changed.add(waiter);
        if (intent.key().equals(activeKey)) activeWaiters = List.copyOf(changed);
        else pendingWaiters = List.copyOf(changed);
    }

    private List<PopupWaiter> currentWaiters(String key) {
        synchronized (lock) { return key.equals(activeKey) ? activeWaiters : List.of(); }
    }

    private static PopupWaiter waiter(Intent intent, Editor editor, PopupKind kind) {
        return new PopupWaiter(editor, intent.document(), intent.file(), intent.documentStamp(),
                intent.position() == null ? -1 : intent.position().offset(), kind,
                kind == PopupKind.PARAMETERS && CodeInsightSettings.getInstance().AUTO_POPUP_PARAMETER_INFO);
    }

    private static Position position(Editor editor) {
        if (editor == null || editor.isDisposed()) return null;
        Document document = editor.getDocument();
        int offset = editor.getCaretModel().getOffset();
        if (offset < 0 || offset > document.getTextLength()) return null;
        int line = document.getLineNumber(offset);
        int column = offset - document.getLineStartOffset(line);
        return new Position(line + 1, column + 1, offset);
    }

    private static boolean splitsSurrogatePair(Document document, int offset) {
        CharSequence text = document.getCharsSequence();
        return offset > 0 && offset < text.length()
                && Character.isHighSurrogate(text.charAt(offset - 1))
                && Character.isLowSurrogate(text.charAt(offset));
    }

    private static List<DocumentStamp> distinctDocuments(List<DocumentStamp> values) {
        IdentityHashMap<Document, DocumentStamp> distinct = new IdentityHashMap<>();
        for (DocumentStamp value : values) distinct.put(value.document(), value);
        return List.copyOf(distinct.values());
    }

    private static List<FileStamp> distinctFiles(List<FileStamp> values) {
        IdentityHashMap<VirtualFile, FileStamp> distinct = new IdentityHashMap<>();
        for (FileStamp value : values) distinct.put(value.file(), value);
        return List.copyOf(distinct.values());
    }

    private boolean appliesToFile(CachedResponse response, PsiFile file) {
        if (file == null || !file.isValid() || file.getVirtualFile() == null) return false;
        String canonical = normalizePath(canonicalPathFor(response.snapshot, file.getVirtualFile()));
        return samePath(response.snapshot.focusPath.toString(), canonical)
                || response.sourceFileKeys.contains(canonical);
    }

    private static String canonicalPathFor(Snapshot snapshot, VirtualFile file) {
        if (file == null) return "";
        String raw = normalizePath(file.getPath());
        return snapshot.canonicalPaths.getOrDefault(raw, Path.of(file.getPath()).toAbsolutePath().normalize().toString());
    }

    private static Set<String> knownPaths(Snapshot snapshot, NagiAssistProtocol.Response response) {
        LinkedHashSet<String> paths = new LinkedHashSet<>();
        paths.add(snapshot.scopePath.toString());
        paths.add(snapshot.focusPath.toString());
        paths.add(snapshot.executable.toString());
        paths.addAll(snapshot.canonicalPaths.values());
        for (String source : response.sourceFiles()) if (!source.startsWith("stdlib:")) paths.add(source);
        return Set.copyOf(paths);
    }

    private static String safeMessage(Throwable exception) {
        String message = exception.getMessage();
        if (message == null || message.isBlank()) return "Nagi compiler assistance failed; no cached or saved facts were used.";
        if (message.length() > 240) message = message.substring(0, 240);
        return "Nagi compiler assistance failed; no cached or saved facts were used. " + message;
    }

    private void setState(State state, String message) {
        if (state == State.FAILED || state == State.UNTRUSTED || state == State.UNSAVED_MANIFEST
                || state == State.LIMIT || state == State.COMPILER_UNAVAILABLE || state == State.CLOSED) presentation.clear();
        this.state = state;
        this.statusMessage = message;
    }

    private boolean trustedForCompiler() {
        try { return !disposed && !project.isDisposed() && TrustedProjects.isProjectTrusted(project); }
        catch (LinkageError | RuntimeException exception) { return false; }
    }

    private void setStateForActive(String key, State state, String message) {
        synchronized (lock) {
            if (key.equals(activeKey)) {
                if (state == State.STALE) presentation.clear();
                setState(state, message);
                if (state == State.FAILED || state == State.UNSAVED_MANIFEST || state == State.LIMIT) {
                    failedKey = key;
                    restartOpenSourcesLater();
                }
            }
        }
    }

    private static Path canonicalExisting(Path path, boolean directory) throws SnapshotBlockedException {
        assertWorkerIo();
        try {
            Path real = path.toRealPath();
            if (directory ? !Files.isDirectory(real) : !Files.isRegularFile(real)) {
                throw new SnapshotBlockedException(State.STALE, "A Nagi source or project path is not an existing file.");
            }
            return real;
        } catch (IOException exception) {
            throw new SnapshotBlockedException(State.STALE, "An editor source path must exist before compiler assistance can run: " + path);
        }
    }

    static Path resolveCompiler(String configured, Path workspace) throws IOException {
        assertWorkerIo();
        String name = SystemInfo.isWindows ? "nagic.exe" : "nagic";
        Path executable;
        if (configured != null && !configured.isBlank()) {
            executable = Path.of(configured.strip());
            if (!executable.isAbsolute()) executable = workspace.resolve(executable).normalize();
            executable = executable.toRealPath();
            if (!Files.isRegularFile(executable) || !Files.isExecutable(executable)) throw new IOException("compiler is not executable");
            return executable;
        }
        String path = System.getenv("PATH");
        if (path == null) throw new IOException("PATH is not set");
        for (String entry : path.split(java.util.regex.Pattern.quote(SystemInfo.isWindows ? ";" : ":"), -1)) {
            Path directory = entry.isBlank() ? workspace : Path.of(entry);
            Path candidate = directory.resolve(name);
            if (Files.isRegularFile(candidate) && Files.isExecutable(candidate)) return candidate.toRealPath();
        }
        throw new IOException("nagic was not found on PATH");
    }

    private static boolean isNagiSource(Path path) {
        String name = path.getFileName() == null ? "" : path.getFileName().toString();
        return name.endsWith(".nagi") || name.endsWith(".low");
    }
    private static boolean isTrackedSource(VirtualFile file) {
        return file != null && file.isValid() && file.isInLocalFileSystem() && !file.isDirectory()
                && ("nagi".equals(file.getExtension()) || "low".equals(file.getExtension()));
    }
    private static boolean isManifest(VirtualFile file) {
        return file != null && file.isValid() && "nagi.toml".equalsIgnoreCase(file.getName());
    }
    static String normalizePath(String path) {
        String value = NagiAssistProtocol.physicalPath(path).replace('\\', '/');
        return SystemInfo.isWindows ? value.toLowerCase(Locale.ROOT) : value;
    }
    private static boolean samePath(String left, String right) {
        return left != null && right != null && normalizePath(left).equals(normalizePath(right));
    }
    private static boolean pathUnder(String candidate, String root) {
        return candidate.equals(root) || candidate.startsWith(root.endsWith("/") ? root : root + "/");
    }

    @Override public void dispose() {
        disposed = true;
        presentation.clear();
        knownImportDirectories = Set.of();
        importQueryFile = null;
        cached = null;
        if (session != null) session.close();
        setState(State.CLOSED, "Project is closed.");
        List<DocumentationTicket> abandoned;
        synchronized (lock) {
            cancelPendingAndActiveLocked();
            abandoned = detachDocumentation(ticket -> true);
        }
        notifyDocumentation(abandoned, false);
        synchronized (documentListeners) {
            for (Map.Entry<Document, ListenerRecord> entry : documentListeners.entrySet()) {
                Disposer.dispose(entry.getValue().subscription());
            }
            documentListeners.clear();
        }
        // Cancel analysis first, then retire the watch on its owning worker.
        // shutdown preserves this queued cleanup; shutdownNow would discard it.
        try { worker.execute(this::clearImportDirectoryWatch); }
        catch (java.util.concurrent.RejectedExecutionException ignored) { }
        worker.shutdown();
    }

    private record ListenerRecord(VirtualFile file, Disposable subscription) {}
    private record ManifestState(Document document, boolean unsaved, long documentStamp, long fileStamp) {}
    @FunctionalInterface
    interface CompilerUnavailablePresenter { void show(Project project, Runnable retry); }
    interface CompilerResolver { Path resolve(String configured, Path workspace) throws IOException; }
    private static final class SnapshotBlockedException extends Exception {
        private final State state;
        private final CompilerUnavailable compilerUnavailable;
        private final DocumentationRecapture documentationRecapture;
        private SnapshotBlockedException(State state, String message) { this(state, message, null, null); }
        private SnapshotBlockedException(State state, String message, CompilerUnavailable compilerUnavailable) {
            this(state, message, compilerUnavailable, null);
        }
        private SnapshotBlockedException(State state, String message, CompilerUnavailable compilerUnavailable,
                                         DocumentationRecapture documentationRecapture) {
            super(message);
            this.state = state;
            this.compilerUnavailable = compilerUnavailable;
            this.documentationRecapture = documentationRecapture;
        }
    }
}
