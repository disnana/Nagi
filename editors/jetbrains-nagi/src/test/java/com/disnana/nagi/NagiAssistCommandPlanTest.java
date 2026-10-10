package com.disnana.nagi;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;

import java.nio.file.Path;
import java.util.List;
import org.junit.Test;

public final class NagiAssistCommandPlanTest {
    @Test public void projectInvocationPreservesManifestEntryAndNativeConfiguration() {
        Path workspace = Path.of(System.getProperty("java.io.tmpdir"), "nagi assist project")
                .toAbsolutePath().normalize();
        Path manifest = workspace.resolve("nagi.toml");
        Path active = workspace.resolve("src").resolve("page.nagi");
        Path configuredCompiler = workspace.resolve("toolchain").resolve("nagic");

        NagiAssistCommandPlan plan = NagiAssistCommandPlan.create(configuredCompiler.toString(), active,
                manifest, workspace, false);

        assertEquals(configuredCompiler.toString(), plan.executable());
        assertEquals(manifest.getParent(), plan.directory());
        assertEquals(List.of("assist", "--project", manifest.toString(), "--editor-input"), plan.arguments());
        assertFalse("project requests must not override the configured entry source",
                plan.arguments().contains(active.toString()));
    }

    @Test public void standaloneInvocationKeepsSourceAsOneArgumentAndUsesNoProjectMode() {
        Path workspace = Path.of(System.getProperty("java.io.tmpdir"), "nagi assist standalone")
                .toAbsolutePath().normalize();
        Path source = workspace.resolve("folder with spaces").resolve("main.nagi");

        NagiAssistCommandPlan plan = NagiAssistCommandPlan.create("tools/nagic", source,
                null, workspace, false);

        assertEquals(workspace.resolve("tools/nagic").toString(), plan.executable());
        assertEquals(source.getParent(), plan.directory());
        assertEquals(List.of("assist", source.toString(), "--no-project", "--editor-input"), plan.arguments());
    }

    @Test public void emptyCompilerSettingUsesPlatformExecutableName() {
        Path workspace = Path.of(System.getProperty("java.io.tmpdir"), "nagi assist default")
                .toAbsolutePath().normalize();
        Path source = workspace.resolve("main.low");

        NagiAssistCommandPlan plan = NagiAssistCommandPlan.create("", source, null, workspace, true);

        assertEquals("nagic.exe", plan.executable());
    }
}
