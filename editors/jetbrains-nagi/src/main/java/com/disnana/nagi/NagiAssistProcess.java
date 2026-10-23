package com.disnana.nagi;

import java.io.ByteArrayOutputStream;
import java.io.BufferedInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.BooleanSupplier;

/** Bounded process transport. It never runs on the EDT and never invokes a shell. */
public final class NagiAssistProcess {
    private static final int MAX_STDERR_BYTES = 1_000_000;

    private NagiAssistProcess() {}

    @FunctionalInterface
    public interface Runner {
        NagiAssistProtocol.Response run(NagiAssistCommandPlan plan, byte[] input, int timeoutSeconds,
                                        Cancellation cancellation, BooleanSupplier mayLaunch) throws Exception;
    }

    public static final class Cancellation {
        private final AtomicBoolean cancelled = new AtomicBoolean();
        private final boolean hasDeadline;
        private final long deadlineNanos;
        private volatile Process process;

        public Cancellation() { hasDeadline = false; deadlineNanos = 0; }
        /** Absolute System.nanoTime deadline, shared across every request in one operation. */
        public Cancellation(long deadlineNanos) { hasDeadline = true; this.deadlineNanos = deadlineNanos; }

        /** Expiry is an I/O failure, distinct from editor cancellation and interruption. */
        void checkDeadline() throws IOException { remainingNanos(Long.MAX_VALUE); }

        long remainingNanos(int timeoutSeconds) throws IOException {
            return remainingNanos(TimeUnit.SECONDS.toNanos(Math.max(1, Math.min(300, timeoutSeconds))));
        }

        private long remainingNanos(long maximum) throws IOException {
            long remaining = hasDeadline ? deadlineNanos - System.nanoTime() : Long.MAX_VALUE;
            if (remaining <= 0) throw new IOException("Nagi editor assistance timed out");
            return Math.min(remaining, maximum);
        }

        public boolean isCancelled() { return cancelled.get() || Thread.currentThread().isInterrupted(); }
        public void attach(Process process) {
            this.process = process;
            if (isCancelled()) terminate(process);
        }
        public void cancel() {
            cancelled.set(true);
            Process active = process;
            // Event listeners may call this on EDT. Killing is nonblocking; the
            // worker's finally block performs the reap before the next request.
            if (active != null && active.isAlive()) active.destroyForcibly();
        }
        public void detach(Process candidate) { if (process == candidate) process = null; }
    }

    /** One compiler per project/command plan, with serialized newline frames.
     * Cancellation kills and reaps the whole session: no old response can be
     * confused with the next request. This is transport reuse, not incremental
     * type checking. The compiler reloads immutable input for every request. */
    public static final class Session implements AutoCloseable {
        private volatile Process process;
        private NagiAssistCommandPlan plan;
        private ExecutableIdentity executableIdentity;
        private BufferedInputStream output;
        private CompletableFuture<byte[]> errors;
        private volatile boolean closed;
        private int launches;
        private record ExecutableIdentity(long size, java.nio.file.attribute.FileTime modified, Object key) {
            static ExecutableIdentity read(String path) throws IOException {
                var attributes = java.nio.file.Files.readAttributes(java.nio.file.Path.of(path), java.nio.file.attribute.BasicFileAttributes.class);
                return new ExecutableIdentity(attributes.size(), attributes.lastModifiedTime(), attributes.fileKey());
            }
        }

        public synchronized NagiAssistProtocol.Response run(NagiAssistCommandPlan requested, byte[] input,
                int timeoutSeconds, Cancellation cancellation, BooleanSupplier mayLaunch) throws Exception {
            if (closed || cancellation.isCancelled()) throw new InterruptedException("editor assistance cancelled before launch");
            try { cancellation.checkDeadline(); }
            catch (IOException expired) { stop(); throw expired; }
            if (!mayLaunch.getAsBoolean()) { stop(); throw new IOException("editor assistance launch guard rejected the request"); }
            if (input.length > NagiAssistProtocol.MAX_INPUT_BYTES) throw new IOException("editor input exceeds its limit");
            ExecutableIdentity identity = ExecutableIdentity.read(requested.executable());
            if (process == null || !process.isAlive() || !requested.equals(plan) || !identity.equals(executableIdentity)) {
                stop();
                List<String> command = new ArrayList<>();
                command.add(requested.executable());
                command.addAll(requested.arguments());
                command.add("--serve");
                cancellation.checkDeadline();
                process = new ProcessBuilder(command).directory(requested.directory().toFile()).start();
                launches++;
                plan = requested;
                executableIdentity = identity;
                output = new BufferedInputStream(process.getInputStream());
                errors = readBounded(process.getErrorStream(), MAX_STDERR_BYTES, process);
            }
            Process current = process;
            cancellation.attach(current);
            try {
                cancellation.checkDeadline();
                if (closed || cancellation.isCancelled() || !mayLaunch.getAsBoolean()
                        || !identity.equals(ExecutableIdentity.read(requested.executable())))
                    throw new InterruptedException("editor assistance cancelled before request");
                BufferedInputStream frames = output;
                CompletableFuture<byte[]> response = new CompletableFuture<>();
                Thread.ofVirtual().name("nagi-assist-frame").start(() -> {
                    try {
                        // A request may be larger than the pipe capacity. Write
                        // and response read are bounded by the same deadline.
                        current.getOutputStream().write(input);
                        current.getOutputStream().write('\n');
                        current.getOutputStream().flush();
                        var frame = new ByteArrayOutputStream();
                        for (int next; (next = frames.read()) != -1;) {
                            if (next == '\n') { response.complete(frame.toByteArray()); return; }
                            if (frame.size() == NagiAssistProtocol.MAX_OUTPUT_BYTES)
                                throw new IOException("editor response exceeds its limit");
                            frame.write(next);
                        }
                        throw new IOException("Nagi assistance ended before responding (startup failure or unsupported protocol). Use published Nagi 0.2.0 from GitHub Releases for assistance. Nagi 0.1.11 supports normal Check/Run only.");
                    } catch (Throwable failure) { response.completeExceptionally(failure); }
                });
                byte[] frame = response.get(cancellation.remainingNanos(timeoutSeconds), TimeUnit.NANOSECONDS);
                cancellation.checkDeadline();
                if (closed || cancellation.isCancelled() || !mayLaunch.getAsBoolean()
                        || !identity.equals(ExecutableIdentity.read(requested.executable())))
                    throw new InterruptedException("editor assistance cancelled after request");
                if (errors.isCompletedExceptionally()) errors.get();
                NagiAssistProtocol.Response decoded = NagiAssistProtocol.decode(frame);
                cancellation.checkDeadline();
                return decoded;
            } catch (java.util.concurrent.TimeoutException failure) {
                stop();
                throw new IOException("Nagi editor assistance timed out", failure);
            } catch (Exception | LinkageError failure) {
                stop();
                throw failure;
            } finally { cancellation.detach(current); }
        }

        synchronized int launches() { return launches; }

        private void stop() {
            Process previous = process;
            process = null;
            plan = null;
            executableIdentity = null;
            output = null;
            if (previous != null) terminate(previous);
        }

        @Override public void close() {
            // Called from project disposal on EDT; never wait here. Worker owns
            // reaping. closed prevents a startup race from retaining a process.
            closed = true;
            Process current = process;
            if (current != null && current.isAlive()) {
                current.destroyForcibly();
                Thread.ofVirtual().name("nagi-assist-reap").start(() -> terminate(current));
            }
        }
    }

    public static NagiAssistProtocol.Response run(NagiAssistCommandPlan plan, byte[] input, int timeoutSeconds,
                                                  Cancellation cancellation, BooleanSupplier mayLaunch) throws Exception {
        if (cancellation.isCancelled()) throw new InterruptedException("editor assistance cancelled before launch");
        cancellation.checkDeadline();
        if (!mayLaunch.getAsBoolean()) throw new IOException("editor assistance launch guard rejected the request");
        List<String> command = new ArrayList<>();
        command.add(plan.executable());
        command.addAll(plan.arguments());
        cancellation.checkDeadline();
        Process process = new ProcessBuilder(command).directory(plan.directory().toFile()).start();
        cancellation.attach(process);
        CompletableFuture<byte[]> stdout = readBounded(process.getInputStream(), NagiAssistProtocol.MAX_OUTPUT_BYTES, process);
        CompletableFuture<byte[]> stderr = readBounded(process.getErrorStream(), MAX_STDERR_BYTES, process);
        try {
            try (OutputStream stream = process.getOutputStream()) { stream.write(input); }
            // The existing one-shot write is synchronous. Check the shared
            // budget after it; do not grant another timeout to subsequent waits.
            boolean finished = process.waitFor(cancellation.remainingNanos(timeoutSeconds), TimeUnit.NANOSECONDS);
            if (!finished) {
                terminate(process);
                throw new IOException("Nagi editor assistance timed out");
            }
            if (cancellation.isCancelled()) throw new InterruptedException("editor assistance cancelled");
            cancellation.checkDeadline();
            byte[] output = stdout.get(cancellation.remainingNanos(2), TimeUnit.NANOSECONDS);
            byte[] errors = stderr.get(cancellation.remainingNanos(2), TimeUnit.NANOSECONDS);
            cancellation.checkDeadline();
            if (process.exitValue() != 0) {
                String detail = new String(errors, StandardCharsets.UTF_8);
                if (detail.length() > 2000) detail = detail.substring(0, 2000);
                throw new IOException("Nagi editor assistance exited with status " + process.exitValue() + (detail.isBlank() ? "" : ": " + detail));
            }
            if (output.length == 0) throw new IOException("Nagi editor assistance returned no response");
            NagiAssistProtocol.Response decoded = NagiAssistProtocol.decode(output);
            cancellation.checkDeadline();
            return decoded;
        } catch (java.util.concurrent.TimeoutException failure) {
            throw new IOException("Nagi editor assistance timed out", failure);
        } finally {
            if (process.isAlive()) terminate(process);
            cancellation.detach(process);
        }
    }

    private static CompletableFuture<byte[]> readBounded(InputStream input, int limit, Process process) {
        CompletableFuture<byte[]> result = new CompletableFuture<>();
        Thread.ofVirtual().name("nagi-assist-output").start(() -> {
            try (input; var output = new ByteArrayOutputStream(Math.min(limit, 16_384))) {
                byte[] buffer = new byte[8192];
                int count;
                int total = 0;
                while ((count = input.read(buffer)) >= 0) {
                    total += count;
                    if (total > limit) {
                        terminate(process);
                        throw new IOException("Nagi editor assistance output exceeded its limit");
                    }
                    output.write(buffer, 0, count);
                }
                result.complete(output.toByteArray());
            } catch (Throwable failure) {
                result.completeExceptionally(failure);
            }
        });
        return result;
    }

    private static void terminate(Process process) {
        if (!process.isAlive()) return;
        process.destroy();
        boolean interrupted = false;
        try {
            if (!process.waitFor(200, TimeUnit.MILLISECONDS)) {
                process.destroyForcibly();
                process.waitFor(2, TimeUnit.SECONDS);
            }
        } catch (InterruptedException exception) {
            interrupted = true;
            process.destroyForcibly();
            try { process.waitFor(2, TimeUnit.SECONDS); }
            catch (InterruptedException ignored) { interrupted = true; }
        }
        if (interrupted) Thread.currentThread().interrupt();
    }
}
