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
}
