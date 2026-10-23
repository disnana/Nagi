package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.ide.trustedProjects.TrustedProjectsListener;
import com.intellij.openapi.Disposable;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.components.Service;
import com.intellij.openapi.progress.ProgressIndicator;
import com.intellij.openapi.progress.ProcessCanceledException;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.project.ProjectManager;
import com.intellij.openapi.project.ProjectManagerListener;
import com.intellij.openapi.vfs.VirtualFileManager;
import com.intellij.openapi.vfs.newvfs.BulkFileListener;
import com.intellij.openapi.vfs.newvfs.events.VFileEvent;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.BasicFileAttributes;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;
import org.jetbrains.annotations.NotNull;

/** Run-only capability preparation. No compiler process or filesystem hashing on the EDT/read action. */
@Service(Service.Level.PROJECT)
public final class NagiRunPreparation implements Disposable {
    static final int CACHE_LIMIT = 8;
    static final int ACTIVE_LIMIT = 8;
    static final int STDOUT_LIMIT = 65_536;
    static final int STDERR_LIMIT = 16_384;
    static final String PROFILE_HELP = "  --profile dev|release   Native build profile (default: release; build/run)";
    // Exact normalized --help from the verified public 0.2.0 distribution. Other old/unknown
    // interfaces need explicit Release; absence of a flag in arbitrary help is not proof.
    private static final String PUBLIC_020_HELP = "2dfd4b127d7b491ce1a726408104cb87da5a73e80230e653d6568d05844ea2da";
    private static final String PUBLIC_011_HELP = "13de0077ade5cb43f9eea20d0f64bc7cc88a621c50166543e9862e8e31b834fb";
    static final String LEGACY_NOTICE = "Nagi Run: actual profile is Release. This compiler does not support Development; use Nagi 0.2.1 or a compiler advertising --profile dev|release.\n";
    private static final String ADVICE = "Could not verify Development support. Select Release in the Nagi Run Configuration or select a compatible compiler in Nagi settings.";
    enum Capability { DEVELOPMENT, LEGACY_RELEASE }
    record Selection(String configured, Path workspace, long epoch) {}
    record Prepared(NagiCommandPlan plan, String notice, Selection selection) {}
    private record Attributes(long size, java.nio.file.attribute.FileTime modified, Object key) {
        static Attributes read(Path path) throws IOException {
            var value = Files.readAttributes(path, BasicFileAttributes.class);
            if (!value.isRegularFile() || !Files.isExecutable(path)) throw new IOException("Compiler is not an executable regular file.");
            return new Attributes(value.size(), value.lastModifiedTime(), value.fileKey());
        }
    }
    private record Identity(Path path, Attributes attributes, String digest) {}
    private final Project project;
    private final long timeoutMillis;
    private final Object lock = new Object();
    private final LinkedHashMap<Identity, Capability> completed = new LinkedHashMap<>(16, .75f, true);
    private final Set<Attempt> active = new HashSet<>();
    private volatile long epoch;
    private volatile boolean disposed;

    public NagiRunPreparation(Project project) { this(project, 5_000); }
    NagiRunPreparation(Project project, long timeoutMillis) {
        this.project = project;
        this.timeoutMillis = timeoutMillis;
        var bus = ApplicationManager.getApplication().getMessageBus().connect(this);
        bus.subscribe(NagiSettings.SETTINGS_CHANGED, this::invalidate);
        bus.subscribe(TrustedProjectsListener.TOPIC, new TrustedProjectsListener() {
            @Override public void onProjectUntrusted(@NotNull Project changed) { if (changed == project) invalidate(); }
        });
        bus.subscribe(ProjectManager.TOPIC, new ProjectManagerListener() {
            @Override public void projectClosing(@NotNull Project changed) { if (changed == project) invalidate(); }
        });
        bus.subscribe(VirtualFileManager.VFS_CHANGES, new BulkFileListener() {
            @Override public void before(@NotNull List<? extends VFileEvent> events) {
                boolean relevant;
                synchronized (lock) {
                    relevant = events.stream().anyMatch(event -> completed.keySet().stream().anyMatch(identity -> affected(event, identity.path()))
                            || active.stream().anyMatch(attempt -> attempt.executable != null && affected(event, attempt.executable)));
                }
                if (relevant) invalidate();
            }
        });
    }
    private static boolean affected(VFileEvent event, Path executable) {
        String path = event.getPath().replace('\\', '/');
        String target = executable.toString().replace('\\', '/');
        if (com.intellij.openapi.util.SystemInfo.isWindows) { path = path.toLowerCase(java.util.Locale.ROOT); target = target.toLowerCase(java.util.Locale.ROOT); }
        return target.equals(path) || target.startsWith(path.endsWith("/") ? path : path + "/");
    }
    static NagiRunPreparation getInstance(Project project) { return project.getService(NagiRunPreparation.class); }
    Selection select(String configured, Path workspace) { return new Selection(configured, workspace, epoch); }
    void checkCurrent(Selection selection) throws ExecutionException {
        if (disposed || project.isDisposed()) throw new ProcessCanceledException();
        if (selection.epoch() != epoch || !java.util.Objects.equals(selection.configured(), NagiSettings.getInstance().getState().compilerPath))
            throw new ExecutionException("Nagi compiler settings or executable changed while preparing Run. Run again.");
        if (!TrustedProjects.isProjectTrusted(project)) throw new ExecutionException("Trust this project before executing the Nagi compiler.");
    }
    Prepared prepare(NagiCommandPlan original, NagiBuildProfile profile, Selection selection,
                     ProgressIndicator indicator, BooleanSupplier mayLaunch) throws ExecutionException {
        ApplicationManager.getApplication().assertIsNonDispatchThread();
        if (ApplicationManager.getApplication().isReadAccessAllowed()) throw new IllegalStateException("Run preparation must not hold a read action.");
        checkCurrent(selection);
        indicator.checkCanceled();
        if (!mayLaunch.getAsBoolean()) throw new ProcessCanceledException();
        if (profile == NagiBuildProfile.RELEASE) return new Prepared(original, "", selection);
        Attempt attempt = new Attempt(selection, indicator, mayLaunch);
        synchronized (lock) {
            if (active.size() >= ACTIVE_LIMIT) throw new ExecutionException("Too many Nagi Development preparations. Wait or select Release.");
            active.add(attempt);
        }
        try {
            attempt.check();
            Path executable = NagiAssistService.resolveCompiler(selection.configured(), selection.workspace());
            attempt.executable = executable;
            Identity identity = identity(executable, attempt);
            Capability capability;
            synchronized (lock) { capability = completed.get(identity); }
            if (capability == null) capability = probe(executable, original.directory(), attempt);
            attempt.check();
            // Do not publish a result for a compiler replaced during help, including same-size/mtime replacements.
            if (!executable.equals(NagiAssistService.resolveCompiler(selection.configured(), selection.workspace()))
                    || !identity.equals(identity(executable, attempt))) throw new IOException("Compiler changed during capability preparation.");
            synchronized (lock) {
                if (selection.epoch() != epoch || disposed) throw new IOException("Compiler selection changed.");
                completed.put(identity, capability);
                while (completed.size() > CACHE_LIMIT) completed.remove(completed.keySet().iterator().next());
            }
            var arguments = new ArrayList<>(original.arguments());
            if (capability == Capability.DEVELOPMENT) { arguments.add("--profile"); arguments.add("dev"); }
            return new Prepared(new NagiCommandPlan(executable.toString(), List.copyOf(arguments), original.directory()),
                    capability == Capability.LEGACY_RELEASE ? LEGACY_NOTICE : "", selection);
        } catch (ProcessCanceledException canceled) { throw canceled; }
        catch (Exception failure) { throw new ExecutionException(ADVICE + "\n" + failure.getMessage(), failure); }
        finally {
            try { attempt.close(); }
            finally { synchronized (lock) { active.remove(attempt); } }
        }
    }
    private static Identity identity(Path executable, Attempt attempt) throws Exception {
        Attributes before = Attributes.read(executable);
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = Files.newInputStream(executable)) {
            byte[] buffer = new byte[16_384];
            for (int count; (count = input.read(buffer)) != -1;) { attempt.check(); digest.update(buffer, 0, count); }
        }
        if (!before.equals(Attributes.read(executable))) throw new IOException("Compiler changed while reading its identity.");
        return new Identity(executable, before, HexFormat.of().formatHex(digest.digest()));
    }
    private Capability probe(Path executable, Path directory, Attempt attempt) throws Exception {
        attempt.check();
        Process process = new ProcessBuilder(executable.toString(), "--help").directory(directory.toFile()).start();
        attempt.process = process;
        process.getOutputStream().close();
        var stdout = read(process.getInputStream(), STDOUT_LIMIT, attempt);
        var stderr = read(process.getErrorStream(), STDERR_LIMIT, attempt);
        long deadline = attempt.deadline;
        try {
            while (true) {
                attempt.check();
                attempt.rememberChildren();
                if (stdout.isCompletedExceptionally()) stdout.get();
                if (stderr.isCompletedExceptionally()) stderr.get();
                long remaining = deadline - System.nanoTime();
                if (remaining <= 0) throw new IOException("Nagi help timed out.");
                if (process.waitFor(Math.max(1, Math.min(50, TimeUnit.NANOSECONDS.toMillis(remaining))), TimeUnit.MILLISECONDS)) break;
            }
            attempt.check();
            byte[] output = stdout.get(Math.max(1, deadline - System.nanoTime()), TimeUnit.NANOSECONDS);
            byte[] errors = stderr.get(Math.max(1, deadline - System.nanoTime()), TimeUnit.NANOSECONDS);
            if (process.exitValue() != 0) throw new IOException("Nagi help exited with status " + process.exitValue() + ".");
            String error = utf8(errors);
            if (!error.isEmpty()) throw new IOException("Nagi help produced unexpected stderr.");
            return classify(utf8(output));
        } finally {
            attempt.close();
            // Closing our pipe ends also releases bounded readers when a child kept a pipe open.
            process.getInputStream().close(); process.getErrorStream().close();
            stdout.handle((value, failure) -> null).get(2, TimeUnit.SECONDS);
            stderr.handle((value, failure) -> null).get(2, TimeUnit.SECONDS);
        }
    }
    static Capability classify(String help) throws Exception {
        String normalized = help.replace("\r\n", "\n");
        if (normalized.chars().anyMatch(value -> value < 32 && value != '\n' || value == 127)) throw new IOException("Malformed Nagi help.");
        String digest = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(normalized.getBytes(StandardCharsets.UTF_8)));
        if (PUBLIC_020_HELP.equals(digest) || PUBLIC_011_HELP.equals(digest)) return Capability.LEGACY_RELEASE;
        String[] lines = normalized.split("\n", -1);
        boolean header = lines.length > 2 && lines[0].matches("Nagi compiler [0-9]+\\.[0-9]+\\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?");
        if (!header || !normalized.contains("\nUsage:\n  nagic <check|lower|build|run|symbols|assist> [SOURCE] [OPTIONS]\n")
                || !normalized.contains("\nCommands:\n") || !normalized.contains("\nOptions:\n")) throw new IOException("Unrecognized Nagi help.");
        long profiles = java.util.Arrays.stream(lines).filter(line -> line.stripLeading().startsWith("--profile")).count();
        int options = normalized.indexOf("\nOptions:\n");
        int notes = normalized.indexOf("\nMap notes:", options);
        int declaration = normalized.indexOf("\n" + PROFILE_HELP + "\n");
        if (profiles == 1 && options == normalized.lastIndexOf("\nOptions:\n")
                && declaration > options && (notes == -1 || declaration < notes)) return Capability.DEVELOPMENT;

        throw new IOException("Unrecognized Nagi profile interface.");
    }
    private static String utf8(byte[] bytes) throws java.nio.charset.CharacterCodingException {
        return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString();
    }
    private static CompletableFuture<byte[]> read(InputStream stream, int limit, Attempt attempt) {
        var result = new CompletableFuture<byte[]>();
        Thread.ofVirtual().name("nagi-run-help-output").start(() -> {
            try (var output = new ByteArrayOutputStream()) {
                byte[] buffer = new byte[4096]; int total = 0;
                for (int count; (count = stream.read(buffer)) != -1;) {
                    total += count;
                    if (total > limit) throw new IOException("Nagi help exceeded its output limit.");
                    output.write(buffer, 0, count);
                }
                result.complete(output.toByteArray());
            } catch (Exception failure) { result.completeExceptionally(failure); attempt.kill(); }
        });
        return result;
    }
    private void invalidate() {
        List<Attempt> cancel;
        synchronized (lock) { epoch++; completed.clear(); cancel = List.copyOf(active); }
        cancel.forEach(Attempt::cancel);
    }
    @Override public void dispose() { disposed = true; invalidate(); }
    int cachedCount() { synchronized (lock) { return completed.size(); } }
    static void stopAndReap(Process current, boolean hasChildren) {
        boolean interrupted = Thread.interrupted();
        try {
            // A waiting parent may reap killed children before it is forcibly stopped.
            try { if (hasChildren) current.waitFor(200, TimeUnit.MILLISECONDS); }
            catch (InterruptedException failure) { interrupted = true; }
            if (current.isAlive()) current.destroyForcibly();
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(2);
            while (current.isAlive()) {
                long remaining = deadline - System.nanoTime();
                if (remaining <= 0) throw new IllegalStateException("Nagi help process did not terminate.");
                try { current.waitFor(remaining, TimeUnit.NANOSECONDS); }
                catch (InterruptedException failure) { interrupted = true; }
            }
        } finally { if (interrupted) Thread.currentThread().interrupt(); }
    }
    private final class Attempt implements AutoCloseable {
        final Selection selection;
        final long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMillis);
        final ProgressIndicator indicator;
        final BooleanSupplier mayLaunch;
        volatile boolean canceled;
        volatile Process process;
        volatile Path executable;
        final Set<ProcessHandle> children = java.util.concurrent.ConcurrentHashMap.newKeySet();
        Attempt(Selection selection, ProgressIndicator indicator, BooleanSupplier mayLaunch) {
            this.selection = selection; this.indicator = indicator; this.mayLaunch = mayLaunch;
        }
        void check() throws ExecutionException {
            indicator.checkCanceled();
            if (canceled || Thread.currentThread().isInterrupted() || !mayLaunch.getAsBoolean()) throw new ProcessCanceledException();
            checkCurrent(selection);
            if (System.nanoTime() >= deadline) throw new ExecutionException("Nagi Development preparation timed out. Select Release or check the compiler.");
        }
        void rememberChildren() { Process current = process; if (current != null && current.isAlive()) current.descendants().forEach(children::add); }
        void cancel() {
            canceled = true;
            kill();
        }
        void kill() {
            Process current = process;
            if (current != null) { rememberChildren(); children.forEach(child -> { if (child.isAlive()) child.destroyForcibly(); }); if (children.isEmpty() && current.isAlive()) current.destroyForcibly(); }
        }
        @Override public void close() {
            Process current = process;
            if (current == null) return;
            rememberChildren();
            children.forEach(child -> { if (child.isAlive()) child.destroyForcibly(); });
            stopAndReap(current, !children.isEmpty());
        }
    }
}
