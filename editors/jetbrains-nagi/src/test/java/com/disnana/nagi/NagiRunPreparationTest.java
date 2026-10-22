package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.progress.EmptyProgressIndicator;
import com.intellij.openapi.progress.ProcessCanceledException;
import com.intellij.openapi.util.Disposer;
import com.intellij.testFramework.PlatformTestUtil;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

public class NagiRunPreparationTest extends BasePlatformTestCase {
    private Path root;
    private String previousCompiler;
    private boolean previousTrust;
    @Override protected void setUp() throws Exception {
        super.setUp();
        root = Files.createTempDirectory("nagi profile probe ");
        previousCompiler = NagiSettings.getInstance().getState().compilerPath;
        previousTrust = TrustedProjects.isProjectTrusted(getProject());
        TrustedProjects.setProjectTrusted(getProject(), true);
    }
    @Override protected void tearDown() throws Exception {
        try {
            NagiSettings.update(previousCompiler, NagiSettings.getInstance().getState().checkTimeoutSeconds);
            TrustedProjects.setProjectTrusted(getProject(), previousTrust);
            try (var paths = Files.walk(root)) {
                for (var path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        } finally { super.tearDown(); }
    }
    public void testActualSelectedCompilerPreparation() throws Exception {
        String compiler = System.getenv("NAGI_TEST_COMPILER");
        org.junit.Assume.assumeTrue("Real compiler fixture is required", compiler != null && !compiler.isBlank());
        // Explicit expected capability is useful for distribution compatibility runs. CI's
        // checkout-built compiler advertises Development; the public 0.2.0 run explicitly selects legacy.
        boolean expectDevelopment = !"legacy".equals(System.getenv("NAGI_TEST_RUN_PROFILE"));
        NagiSettings.update(compiler, 30);
        var service = service(5000);
        var prepared = await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        assertEquals(expectDevelopment, prepared.plan().arguments().contains("--profile"));
        assertEquals(expectDevelopment ? "" : NagiRunPreparation.LEGACY_NOTICE, prepared.notice());
        assertEquals(Path.of(compiler).toRealPath().toString(), prepared.plan().executable());
        var release = await(prepare(service, NagiBuildProfile.RELEASE, new EmptyProgressIndicator()));
        assertFalse(release.plan().arguments().contains("--profile"));
        System.out.println("NAGI_REAL_PROFILE " + prepared.plan().executable() + " development=" + expectDevelopment);
    }
    public void testRealHelpDistinguishesSameVersionAndRejectsUnknownOrMisleadingProfile() throws Exception {
        try (var input = getClass().getResourceAsStream("/run-help-public-0.1.11.txt")) {
            assertNotNull(input);
            assertEquals(NagiRunPreparation.Capability.LEGACY_RELEASE,
                    NagiRunPreparation.classify(new String(input.readAllBytes(), java.nio.charset.StandardCharsets.UTF_8)));
        }
        String development = NagiRunTestCompiler.help(true), legacy = NagiRunTestCompiler.help(false);
        assertEquals(development.lines().findFirst().orElseThrow(), legacy.lines().findFirst().orElseThrow());
        assertEquals(NagiRunPreparation.Capability.DEVELOPMENT, NagiRunPreparation.classify(development));
        assertEquals(NagiRunPreparation.Capability.LEGACY_RELEASE, NagiRunPreparation.classify(legacy));
        assertEquals(NagiRunPreparation.Capability.LEGACY_RELEASE, NagiRunPreparation.classify(legacy.replace("\n", "\r\n")));
        for (String unknown : List.of("some tool\n" + development, legacy + "unexpected\n", development + NagiRunPreparation.PROFILE_HELP + "\n",
                development.replace(NagiRunPreparation.PROFILE_HELP, "  --profile release  Release only"), development + "\u001b[31m", development.replace(NagiRunPreparation.PROFILE_HELP + "\n", "").replace("Map notes:\n", "Map notes:\n" + NagiRunPreparation.PROFILE_HELP + "\n"))) {
            try { NagiRunPreparation.classify(unknown); fail("Unknown help was accepted"); } catch (java.io.IOException expected) { /* fail closed */ }
        }
    }
    public void testDevelopmentIsDefaultAndReleaseRoundTripsWithoutLosingExplicitChoice() throws Exception {
        var factory = NagiRunConfigurationType.getInstance().getConfigurationFactories()[0];
        var configuration = (NagiRunConfiguration) factory.createTemplateConfiguration(getProject());
        assertEquals(NagiBuildProfile.DEVELOPMENT, configuration.getBuildProfile());
        configuration.setBuildProfile(NagiBuildProfile.RELEASE);
        var xml = new org.jdom.Element("configuration"); configuration.writeExternal(xml);
        var restored = (NagiRunConfiguration) factory.createTemplateConfiguration(getProject()); restored.readExternal(xml);
        assertEquals(NagiBuildProfile.RELEASE, restored.getBuildProfile());
        assertEquals(NagiBuildProfile.RELEASE, ((NagiRunConfiguration) restored.clone()).getBuildProfile());
        xml.removeAttribute("buildProfile"); restored.readExternal(xml);
        assertEquals(NagiBuildProfile.DEVELOPMENT, restored.getBuildProfile());
        Path source = Files.writeString(root.resolve("main.low"), "fn main() {}\n");
        restored.setSourcePath(source.toString()); xml.setAttribute("buildProfile", "untrusted-flag"); restored.readExternal(xml);
        restored.setSourcePath(source.toString());
        try { restored.checkConfiguration(); fail("Unknown persisted profile accepted"); }
        catch (com.intellij.execution.configurations.RuntimeConfigurationException expected) { assertTrue(expected.getMessage().contains("profile")); }
    }
    public void testVerifiedDevelopmentReusesCacheButExplicitReleaseNeverProbes() throws Exception {
        Path compiler = compiler(NagiRunTestCompiler.help(true));
        var service = service(5000);
        var first = await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        assertEquals(List.of("run", root.resolve("main.nagi").toString(), "--out", root.resolve("out").toString(), "--profile", "dev"), first.plan().arguments());
        assertEquals("", first.notice());
        assertEquals(1, probeCount());
        var second = await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        assertEquals(first.plan(), second.plan()); assertEquals(1, probeCount());
        var release = await(prepare(service, NagiBuildProfile.RELEASE, new EmptyProgressIndicator()));
        assertFalse(release.plan().arguments().contains("--profile")); assertEquals(1, probeCount());
        assertEquals(compiler.toString(), first.plan().executable());
    }
    public void testPublicLegacyFallsBackVisiblyAndCachesOnlyVerifiedCompletedResult() throws Exception {
        compiler(NagiRunTestCompiler.help(false)); var service = service(5000);
        var prepared = await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        assertFalse(prepared.plan().arguments().contains("--profile"));
        assertTrue(prepared.notice().contains("actual profile is Release"));
        await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator())); assertEquals(1, probeCount());
    }
    public void testReleaseAcceptsUnknownCompilerWithoutHelpOrProfileFlag() throws Exception {
        executable("#!/bin/sh\nexit 77\n");
        var prepared = await(prepare(service(100), NagiBuildProfile.RELEASE, new EmptyProgressIndicator()));
        assertFalse(prepared.plan().arguments().contains("--profile")); assertFalse(Files.exists(root.resolve("probes")));
    }
    public void testMalformedNonzeroInvalidUtf8AndBoundedOutputFailWithoutCaching() throws Exception {
        org.junit.Assume.assumeFalse(com.intellij.openapi.util.SystemInfo.isWindows);
        for (String body : List.of("printf unknown", "exit 7", "printf '\\377'", "head -c 65537 /dev/zero", "head -c 16385 /dev/zero >&2")) {
            executable("#!/bin/sh\n" + body + "\n"); var service = service(500);
            assertPreparationFails(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
            assertEquals(0, service.cachedCount());
        }
    }
    public void testIdentityHashDetectsSameSizeSameTimestampReplacement() throws Exception {
        Path compiler = compiler(NagiRunTestCompiler.help(true)); var service = service(5000);
        await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        FileTime time = Files.getLastModifiedTime(compiler);
        String before = Files.readString(compiler);
        String after = before.replace("--profile dev|release", "--profile bad|release");
        assertEquals(before.length(), after.length());
        Files.writeString(compiler, after); Files.setLastModifiedTime(compiler, time);
        assertPreparationFails(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        assertEquals(2, probeCount());
    }
    public void testSettingsChangeClearsCacheAndInvalidatesCapturedSelection() throws Exception {
        compiler(NagiRunTestCompiler.help(true)); var service = service(5000);
        await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
        var selection = service.select(NagiSettings.getInstance().getState().compilerPath, root);
        NagiSettings.update(selection.configured(), 30);
        assertEquals(0, service.cachedCount());
        try { service.checkCurrent(selection); fail("Old selection remained current"); } catch (ExecutionException expected) { /* invalid */ }
        await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator())); assertEquals(2, probeCount());
    }
    public void testCompletedIdentityCacheIsBoundedAndEvictsOldest() throws Exception {
        Path compiler = compiler(NagiRunTestCompiler.help(true)); var service = service(5000);
        String script = Files.readString(compiler);
        for (int index = 0; index <= NagiRunPreparation.CACHE_LIMIT; index++) {
            Files.writeString(compiler, script + "# identity " + index + "\n");
            await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator()));
            assertTrue(service.cachedCount() <= NagiRunPreparation.CACHE_LIMIT);
        }
        assertEquals(NagiRunPreparation.CACHE_LIMIT, service.cachedCount());
        assertEquals(NagiRunPreparation.CACHE_LIMIT + 1, probeCount());
    }
    public void testReplacementDuringProbeDoesNotPublishOrCacheItsOldHelp() throws Exception {
        Path compiler = compiler(NagiRunTestCompiler.help(true));
        Files.writeString(compiler, Files.readString(compiler).replace("cat <<", "while [ ! -e '" + quote(root.resolve("release")) + "' ]; do sleep 0.02; done\ncat <<"));
        var service = service(5000); var pending = prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator());
        PlatformTestUtil.waitWithEventsDispatching("Probe not started", () -> Files.exists(root.resolve("probes")), 5);
        Files.writeString(compiler, "#!/bin/sh\nprintf unknown\n");
        Files.writeString(root.resolve("release"), "go");
        assertPreparationFails(pending); assertEquals(0, service.cachedCount());
    }
    public void testInterruptedChildReapStillForcesRootTerminationAndRestoresInterrupt() throws Exception {
        var forced = new java.util.concurrent.atomic.AtomicBoolean();
        Process process = new Process() {
            private boolean alive = true;
            @Override public java.io.OutputStream getOutputStream() { return java.io.OutputStream.nullOutputStream(); }
            @Override public java.io.InputStream getInputStream() { return java.io.InputStream.nullInputStream(); }
            @Override public java.io.InputStream getErrorStream() { return java.io.InputStream.nullInputStream(); }
            @Override public int waitFor() { alive = false; return 0; }
            @Override public boolean waitFor(long timeout, TimeUnit unit) throws InterruptedException { throw new InterruptedException("injected during child reap"); }
            @Override public int exitValue() { return 0; }
            @Override public boolean isAlive() { return alive; }
            @Override public void destroy() { alive = false; }
            @Override public Process destroyForcibly() { forced.set(true); alive = false; return this; }
        };
        var done = new CompletableFuture<Boolean>();
        Thread.ofVirtual().start(() -> {
            try { NagiRunPreparation.stopAndReap(process, true); done.complete(Thread.currentThread().isInterrupted()); }
            catch (Throwable failure) { done.completeExceptionally(failure); }
            finally { Thread.interrupted(); }
        });
        assertTrue(await(done)); assertTrue(forced.get()); assertFalse(process.isAlive());
    }
    public void testTimeoutKillsAndReapsOwnedProbeAndChild() throws Exception { heldProbe("timeout"); }
    public void testCancellationKillsAndReapsOwnedProbeAndChild() throws Exception { heldProbe("cancel"); }
    public void testTrustLossKillsProbeAndClearsAuthority() throws Exception { heldProbe("trust"); }
    public void testSettingsChangeKillsProbe() throws Exception { heldProbe("settings"); }
    public void testServiceDisposalKillsProbe() throws Exception { heldProbe("dispose"); }
    private void heldProbe(String event) throws Exception {
        org.junit.Assume.assumeFalse(com.intellij.openapi.util.SystemInfo.isWindows);
        executable("#!/bin/sh\nprintf '%s' $$ > '" + quote(root.resolve("pid")) + "'\nsleep 300 &\nprintf '%s' $! > '"
                + quote(root.resolve("child")) + "'\nwait\n");
        var service = service(event.equals("timeout") ? 250 : 5000);
        var indicator = new EmptyProgressIndicator();
        var pending = prepare(service, NagiBuildProfile.DEVELOPMENT, indicator);
        PlatformTestUtil.waitWithEventsDispatching("Probe did not start", () -> Files.exists(root.resolve("child")), 5);
        long pid = Long.parseLong(Files.readString(root.resolve("pid"))), child = Long.parseLong(Files.readString(root.resolve("child")));
        switch (event) {
            case "cancel" -> indicator.cancel();
            case "trust" -> TrustedProjects.setProjectTrusted(getProject(), false);
            case "settings" -> NagiSettings.update("changed-compiler", 30);
            case "dispose" -> Disposer.dispose(service);
            default -> {}
        }
        Throwable failure = awaitFailure(pending);
        if (event.equals("timeout")) assertInstanceOf(failure, ExecutionException.class);
        else assertInstanceOf(failure, ProcessCanceledException.class);
        PlatformTestUtil.waitWithEventsDispatching("Probe root was not reaped", () -> ProcessHandle.of(pid).map(handle -> !handle.isAlive()).orElse(true), 5);
        PlatformTestUtil.waitWithEventsDispatching("Probe child remained alive", () -> ProcessHandle.of(child).map(handle -> !handle.isAlive()).orElse(true), 5);
        assertEquals(0, service.cachedCount());
    }
    public void testSimultaneousMissesHaveIndependentOwnershipThenReuseCompletedCache() throws Exception {
        Path compiler = compiler(NagiRunTestCompiler.help(true));
        String script = Files.readString(compiler).replace("cat <<", "while [ ! -e '" + quote(root.resolve("release")) + "' ]; do sleep 0.02; done\ncat <<");
        Files.writeString(compiler, script); var service = service(5000);
        var canceled = new EmptyProgressIndicator(); var first = prepare(service, NagiBuildProfile.DEVELOPMENT, canceled);
        var second = prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator());
        PlatformTestUtil.waitWithEventsDispatching("Both independent probes did not start", () -> { try { return probeCount() == 2; } catch (Exception failure) { return false; } }, 5);
        canceled.cancel(); awaitFailure(first); assertFalse(second.isDone());
        Files.writeString(root.resolve("release"), "go"); await(second);
        await(prepare(service, NagiBuildProfile.DEVELOPMENT, new EmptyProgressIndicator())); assertEquals(2, probeCount());
    }
    private NagiRunPreparation service(long timeoutMillis) {
        var service = new NagiRunPreparation(getProject(), timeoutMillis); Disposer.register(getTestRootDisposable(), service); return service;
    }
    private Path compiler(String help) throws Exception {
        return executable("#!/bin/sh\nprintf 'probe\\n' >> '" + quote(root.resolve("probes")) + "'\ncat <<'HELP'\n" + help + "HELP\n");
    }
    private Path executable(String script) throws Exception {
        org.junit.Assume.assumeFalse("Fake executable uses POSIX", com.intellij.openapi.util.SystemInfo.isWindows);
        Path compiler = root.resolve("nagic"); Files.writeString(compiler, script); assertTrue(compiler.toFile().setExecutable(true));
        NagiSettings.update(compiler.toString(), 30); return compiler;
    }
    private int probeCount() throws Exception { return Files.exists(root.resolve("probes")) ? Files.readAllLines(root.resolve("probes")).size() : 0; }
    private CompletableFuture<NagiRunPreparation.Prepared> prepare(NagiRunPreparation service, NagiBuildProfile profile, EmptyProgressIndicator indicator) {
        var selection = service.select(NagiSettings.getInstance().getState().compilerPath, root);
        var plan = new NagiCommandPlan(selection.configured(), List.of("run", root.resolve("main.nagi").toString(), "--out", root.resolve("out").toString()), root);
        var result = new CompletableFuture<NagiRunPreparation.Prepared>();
        Thread.ofVirtual().name("nagi-profile-test").start(() -> {
            try { result.complete(service.prepare(plan, profile, selection, indicator, () -> true)); }
            catch (Throwable failure) { result.completeExceptionally(failure); }
        });
        return result;
    }
    private <T> T await(CompletableFuture<T> pending) throws Exception {
        PlatformTestUtil.waitWithEventsDispatching("Run preparation did not finish", pending::isDone, 10); return pending.get(1, TimeUnit.SECONDS);
    }
    private void assertPreparationFails(CompletableFuture<?> pending) throws Exception {
        Throwable failure = awaitFailure(pending);
        assertInstanceOf(failure, ExecutionException.class); assertTrue(failure.getMessage().contains("Select Release"));
    }
    private Throwable awaitFailure(CompletableFuture<?> pending) throws Exception {
        try { await(pending); fail("Preparation unexpectedly succeeded"); return null; }
        catch (java.util.concurrent.ExecutionException expected) { return expected.getCause(); }
        catch (java.util.concurrent.CancellationException expected) { return expected; }
    }
    private static String quote(Path path) { return path.toString().replace("'", "'\"'\"'"); }
}
