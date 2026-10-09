package com.disnana.nagi;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.IOException;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.Test;

public final class NagiAssistProcessTest {
    @Test public void deniedLaunchGuardRejectsBeforeStartingAnExecutable() {
        Path directory = Path.of(System.getProperty("java.io.tmpdir")).toAbsolutePath().normalize();
        NagiAssistCommandPlan plan = new NagiAssistCommandPlan(
                directory.resolve("executable-that-must-not-run").toString(), List.of("assist"), directory);
        AtomicBoolean checked = new AtomicBoolean();

        IOException failure = assertThrows(IOException.class, () -> NagiAssistProcess.run(plan,
                new byte[0], 1, new NagiAssistProcess.Cancellation(), () -> {
                    checked.set(true);
                    return false;
                }));

        assertTrue("unexpected process failure: " + failure.getMessage(), failure.getMessage().contains("launch guard"));
        assertTrue("the launch guard must be checked before process creation", checked.get());
    }

    @Test public void cancellationBeforeLaunchNeverEvaluatesTheLaunchGuard() {
        Path directory = Path.of(System.getProperty("java.io.tmpdir")).toAbsolutePath().normalize();
        NagiAssistCommandPlan plan = new NagiAssistCommandPlan(
                directory.resolve("executable-that-must-not-run").toString(), List.of("assist"), directory);
        NagiAssistProcess.Cancellation cancellation = new NagiAssistProcess.Cancellation();
        cancellation.cancel();
        AtomicBoolean checked = new AtomicBoolean();

        assertThrows(InterruptedException.class, () -> NagiAssistProcess.run(plan,
                new byte[0], 1, cancellation, () -> {
                    checked.set(true);
                    return true;
                }));

        assertFalse("a cancelled request must exit before launch authorization", checked.get());
    }
    @Test public void persistentTransportReusesCompilerAndNeverReusesOverlayFacts() throws Exception {
        String compiler = System.getenv("NAGI_TEST_COMPILER");
        org.junit.Assume.assumeTrue("matching compiler required", compiler != null && !compiler.isBlank());
        Path directory = java.nio.file.Files.createTempDirectory("nagi persistent assistance ");
        Path source = directory.resolve("main.nagi");
        java.nio.file.Files.writeString(source, "def main():\n    saved = True\n");
        var session = new NagiAssistProcess.Session();
        try {
            var plan = NagiAssistCommandPlan.create(compiler, source, null, directory, false);
            for (String name : List.of("fresh", "latest")) {
                byte[] request = NagiAssistProtocol.encode(new NagiAssistProtocol.Request(
                        List.of(new NagiAssistProtocol.Overlay(source.toString(),
                                "def main():\n    " + name + " = True\n    la\n")),
                        new NagiAssistProtocol.Query(source.toString(), 3, 7)));
                var response = session.run(plan, request, 15, new NagiAssistProcess.Cancellation(), () -> true);
                org.junit.Assert.assertTrue(response.completions().stream().anyMatch(item -> item.name().equals(name)));
                org.junit.Assert.assertFalse(response.completions().stream().anyMatch(item -> item.name().equals("saved")));
                if (name.equals("latest")) assertFalse(response.completions().stream().anyMatch(item -> item.name().equals("fresh")));
            }
            org.junit.Assert.assertEquals(1, session.launches());
            assertThrows(IOException.class, () -> session.run(plan, "{}".getBytes(), 1,
                    new NagiAssistProcess.Cancellation(), () -> false));
        } finally {
            session.close();
            java.nio.file.Files.deleteIfExists(source);
            java.nio.file.Files.deleteIfExists(directory);
        }
    }

    /** Small local protocol peer; no compiler work or external resources. */
    public static final class PendingPeer {
        public static void main(String[] args) throws Exception {
            var input = new java.io.BufferedReader(new java.io.InputStreamReader(System.in));
            if (input.readLine() == null) return;
            java.nio.file.Files.writeString(Path.of(args[0]), Long.toString(ProcessHandle.current().pid()));
            // Keep the request pending until the parent terminates this peer.
            input.readLine();
        }
    }

    @Test public void cancellationTerminatesAndReapsPendingSession() throws Exception {
        pendingSession(false);
    }

    @Test public void timeoutTerminatesAndReapsPendingSession() throws Exception {
        pendingSession(true);
    }

    private static void pendingSession(boolean timeout) throws Exception {
        Path directory = java.nio.file.Files.createTempDirectory("nagi pending assistance ");
        Path marker = directory.resolve("started.pid");
        String javaExecutable = Path.of(System.getProperty("java.home"), "bin", "java").toString();
        var resource = PendingPeer.class.getResource("NagiAssistProcessTest$PendingPeer.class");
        org.junit.Assert.assertNotNull(resource);
        Path classesPath = Path.of(resource.toURI());
        // IDEA's isolated test loader does not publish a CodeSource location.
        for (int part = 0; part < PendingPeer.class.getName().split("\\.").length; part++) classesPath = classesPath.getParent();
        String classes = classesPath.toString();
        var plan = new NagiAssistCommandPlan(javaExecutable, List.of("-cp", classes, PendingPeer.class.getName(), marker.toString()), directory);
        var session = new NagiAssistProcess.Session();
        var cancellation = new NagiAssistProcess.Cancellation();
        var result = new java.util.concurrent.CompletableFuture<Throwable>();
        Thread.ofVirtual().start(() -> {
            try { session.run(plan, "{}".getBytes(), timeout ? 1 : 15, cancellation, () -> true); result.complete(null); }
            catch (Throwable failure) { result.complete(failure); }
        });
        try {
            long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(5);
            while (!java.nio.file.Files.exists(marker) && System.nanoTime() < deadline) Thread.sleep(5);
            assertTrue("the protocol peer must have received the request", java.nio.file.Files.exists(marker));
            long pid = Long.parseLong(java.nio.file.Files.readString(marker));
            if (!timeout) cancellation.cancel();
            Throwable failure = result.get(5, java.util.concurrent.TimeUnit.SECONDS);
            org.junit.Assert.assertNotNull("pending request cannot succeed", failure);
            if (timeout) assertTrue(failure.getMessage(), failure.getMessage().contains("timed out"));
            assertFalse("terminated peer must be reaped before run returns", ProcessHandle.of(pid).map(ProcessHandle::isAlive).orElse(false));
        } finally {
            cancellation.cancel();
            session.close();
            java.nio.file.Files.deleteIfExists(marker);
            java.nio.file.Files.deleteIfExists(directory);
        }
    }
}
