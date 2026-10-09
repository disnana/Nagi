package com.disnana.nagi;

import java.io.ByteArrayOutputStream;
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
        private volatile Process process;

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

    public static NagiAssistProtocol.Response run(NagiAssistCommandPlan plan, byte[] input, int timeoutSeconds,
                                                  Cancellation cancellation, BooleanSupplier mayLaunch) throws Exception {
        if (cancellation.isCancelled()) throw new InterruptedException("editor assistance cancelled before launch");
        if (!mayLaunch.getAsBoolean()) throw new IOException("editor assistance launch guard rejected the request");
        List<String> command = new ArrayList<>();
        command.add(plan.executable());
        command.addAll(plan.arguments());
        Process process = new ProcessBuilder(command).directory(plan.directory().toFile()).start();
        cancellation.attach(process);
        CompletableFuture<byte[]> stdout = readBounded(process.getInputStream(), NagiAssistProtocol.MAX_OUTPUT_BYTES, process);
        CompletableFuture<byte[]> stderr = readBounded(process.getErrorStream(), MAX_STDERR_BYTES, process);
        try {
            try (OutputStream stream = process.getOutputStream()) { stream.write(input); }
            boolean finished = process.waitFor(Math.max(1, Math.min(300, timeoutSeconds)), TimeUnit.SECONDS);
            if (!finished) {
                terminate(process);
                throw new IOException("Nagi editor assistance timed out");
            }
            if (cancellation.isCancelled()) throw new InterruptedException("editor assistance cancelled");
            byte[] output = stdout.get(2, TimeUnit.SECONDS);
            byte[] errors = stderr.get(2, TimeUnit.SECONDS);
            if (process.exitValue() != 0) {
                String detail = new String(errors, StandardCharsets.UTF_8);
                if (detail.length() > 2000) detail = detail.substring(0, 2000);
                throw new IOException("Nagi editor assistance exited with status " + process.exitValue() + (detail.isBlank() ? "" : ": " + detail));
            }
            if (output.length == 0) throw new IOException("Nagi editor assistance returned no response");
            return NagiAssistProtocol.decode(output);
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
