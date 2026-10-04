package com.disnana.nagi;

import com.intellij.execution.configurations.GeneralCommandLine;
import com.intellij.execution.process.OSProcessHandler;
import org.junit.Test;
import java.nio.file.Path;
import java.nio.file.Files;
import java.util.concurrent.TimeUnit;
import static org.junit.Assert.*;

public class NagiProcessTest {
    @Test public void cancelledStartupStopsARealProcessWithoutAConsole() throws Exception {
        String name = System.getProperty("os.name").startsWith("Windows") ? "java.exe" : "java";
        Path directory = Files.createTempDirectory("nagi-process-test");
        Path source = directory.resolve("WaitingChild.java");
        Files.writeString(source, "class WaitingChild { public static void main(String[] args) throws Exception { Thread.sleep(300000); } }");
        var command = new GeneralCommandLine(Path.of(System.getProperty("java.home"), "bin", name).toString()).withParameters(source.toString());
        var handler = new OSProcessHandler(command);
        handler.setShouldDestroyProcessRecursively(true);
        try {
            assertTrue(handler.getProcess().isAlive());
            NagiCompilerAction.stopBeforeConsole(handler);
            assertTrue("cancelled startup must not leave an orphan", handler.waitFor(5000));
            assertFalse(handler.getProcess().isAlive());
        } finally {
            if (handler.getProcess().isAlive()) handler.getProcess().destroyForcibly();
            Files.deleteIfExists(source);
            Files.deleteIfExists(directory);
        }
    }
}
