package com.disnana.nagi;

import org.junit.Assume;
import org.junit.Test;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.concurrent.TimeUnit;
import static org.junit.Assert.*;

/** Optional real-compiler smoke; editor-only CI does not need to build Rust. */
public class NagiCompilerIntegrationTest {
    @Test public void commandPlanChecksHighLowAndProjectWithTheRealCompiler() throws Exception {
        String executable = System.getenv("NAGI_TEST_COMPILER");
        Assume.assumeTrue("Set NAGI_TEST_COMPILER to exercise an installed Nagi compiler", executable != null && !executable.isBlank());
        Path directory = Files.createTempDirectory("nagi compiler smoke ");
        try {
            Path high = directory.resolve("main.nagi");
            Path low = directory.resolve("main.low");
            Files.writeString(high, "def main():\n    print(42)\n");
            Files.writeString(low, "fn main() {\n    print(42);\n}\n");
            check(NagiCommandPlan.create(executable, "check", high, directory, directory.resolve("out-high"), false));
            check(NagiCommandPlan.create(executable, "check", low, directory, directory.resolve("out-low"), false));
            Files.writeString(directory.resolve("nagi.toml"), "entry = \"main.nagi\"\n");
            check(NagiCommandPlan.create(executable, "check", high, directory, directory.resolve("out-project"), false));
        } finally {
            try (var paths = Files.walk(directory)) {
                for (var path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }
    private static void check(NagiCommandPlan plan) throws Exception {
        var arguments = new ArrayList<String>();
        arguments.add(plan.executable());
        arguments.addAll(plan.arguments());
        var process = new ProcessBuilder(arguments).directory(plan.directory().toFile()).redirectErrorStream(true).start();
        try {
            assertTrue("compiler check timed out", process.waitFor(15, TimeUnit.SECONDS));
            String output = new String(process.getInputStream().readAllBytes(), java.nio.charset.StandardCharsets.UTF_8);
            assertEquals(output, 0, process.exitValue());
        } finally {
            if (process.isAlive()) { process.destroyForcibly(); process.waitFor(5, TimeUnit.SECONDS); }
        }
    }
}
