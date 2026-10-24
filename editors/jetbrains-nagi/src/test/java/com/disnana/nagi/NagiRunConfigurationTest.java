package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.Executor;
import com.intellij.execution.RunManager;
import com.intellij.execution.configurations.RunnerSettings;
import com.intellij.execution.executors.DefaultRunExecutor;
import com.intellij.execution.process.OSProcessHandler;
import com.intellij.execution.runners.ExecutionEnvironment;
import com.intellij.execution.runners.ProgramRunner;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.ui.TestDialogManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.psi.PsiManager;
import com.intellij.testFramework.PlatformTestUtil;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import org.jdom.Element;
import org.jetbrains.annotations.NotNull;

public class NagiRunConfigurationTest extends BasePlatformTestCase {
    public void testRunConfigurationPersistsAndCopiesItsSourcePath() throws Exception {
        Path root = Files.createTempDirectory("nagi run configuration state ");
        try {
            Path source = Files.writeString(root.resolve("main.nagi"), "def main():\n    print(1)\n");
            NagiRunConfiguration configuration = newConfiguration();
            configuration.setSourcePath(source.toString());

            Element serialized = new Element("configuration");
            configuration.writeExternal(serialized);
            NagiRunConfiguration restored = newConfiguration();
            restored.readExternal(serialized);

            assertEquals(source.toString(), restored.getSourcePath());
            assertEquals(source.toString(), ((NagiRunConfiguration) restored.clone()).getSourcePath());
        } finally {
            deleteTree(root);
        }
    }

    public void testProducerCreatesConfigurationsForHighAndLowFilesOnly() throws Exception {
        Path root = Files.createTempDirectory("nagi run producer ");
        try {
            for (String name : List.of("main.nagi", "main.low")) {
                Path path = Files.writeString(root.resolve(name), " ");
                var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
                assertNotNull(virtualFile);
                var psiFile = PsiManager.getInstance(getProject()).findFile(virtualFile);
                assertNotNull(psiFile);
                var location = psiFile.findElementAt(0);
                assertNotNull(location);

                var context = new com.intellij.execution.actions.ConfigurationContext(location);
                var producer = new NagiRunConfigurationProducer();
                var produced = producer.createConfigurationFromContext(context);
                assertNotNull(name, produced);
                assertEquals(path.toAbsolutePath().normalize().toString(),
                        ((NagiRunConfiguration) produced.getConfiguration()).getSourcePath());
                assertTrue(producer.isConfigurationFromContext(
                        (NagiRunConfiguration) produced.getConfiguration(), context));
            }

            Path textPath = Files.writeString(root.resolve("plain.txt"), "text");
            var textFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(textPath);
            assertNotNull(textFile);
            var textPsi = PsiManager.getInstance(getProject()).findFile(textFile);
            assertNotNull(textPsi);
            assertNull(new NagiRunConfigurationProducer().createConfigurationFromContext(
                    new com.intellij.execution.actions.ConfigurationContext(textPsi.findElementAt(0))));
        } finally {
            deleteTree(root);
        }
    }

    public void testRunStateSavesInputsUsesProjectArgumentsAndCanStopTheCompiler() throws Exception {
        runWithHeldBarrier(Preparation.SUCCESS);
    }

    public void testLegacyCompilerFallbackUsesReleaseAndShowsActualProfileInConsole() throws Exception {
        runWithHeldBarrier(Preparation.LEGACY_SUCCESS);
    }

    public void testAcceptedDescriptorSurvivesEnvironmentDisposalDuringSuccessCallback() throws Exception {
        runWithHeldBarrier(Preparation.ACCEPTED_HANDOFF);
    }

    public void testProjectDisposalDuringSuccessCallbackStillStopsTheHandoffProcess() throws Exception {
        runWithHeldBarrier(Preparation.HANDOFF_PROJECT_DISPOSE);
    }

    public void testSelectedRunnerIsNagiOnlyAndRunOnly() {
        var runner = ProgramRunner.getRunner(DefaultRunExecutor.EXECUTOR_ID, newConfiguration());
        assertTrue("The registered Nagi async runner must precede the default runner", runner instanceof NagiProgramRunner);
        assertFalse(runner.canRun("Debug", newConfiguration()));
        assertFalse(runner.canRun(DefaultRunExecutor.EXECUTOR_ID, new com.intellij.execution.configurations.RunProfile() {
            @Override public String getName() { return "unrelated"; }
            @Override public javax.swing.Icon getIcon() { return null; }
            @Override public com.intellij.execution.configurations.RunProfileState getState(
                    Executor executor, ExecutionEnvironment environment) { return null; }
        }));
    }

    public void testCompilerSettingChangeDuringPreparationDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.SETTINGS_CHANGE); }
    public void testProfileChangeDuringPreparationDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.PROFILE_CHANGE); }
    public void testCompilerReplacementDuringPreparationDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.COMPILER_SWAP); }

    public void testSaveIOExceptionDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.IO_FAILURE); }
    public void testSaveCancellationDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.PCE); }
    public void testTrustLossDuringSaveDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.TRUST_LOSS); }
    public void testStartupTaskCancellationDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.CANCEL); }
    public void testDisposedEnvironmentDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.DISPOSE_ENVIRONMENT); }
    public void testSupportedProjectCloseCancelsStartupBeforeDisposal() throws Exception { runWithHeldBarrier(Preparation.CLOSE_PROJECT); }
    public void testDirectProjectDisposalSettlesWithoutContentAndDoesNotStartCompiler() throws Exception { runWithHeldBarrier(Preparation.DISPOSE_PROJECT); }
    public void testEditsDuringSaveRequireAnotherRun() throws Exception { runWithHeldBarrier(Preparation.EDIT); }

    public void testConsoleCreationFailureStopsTheCreatedProcess() throws Exception { runWithHeldBarrier(Preparation.CONSOLE_FAILURE); }
    public void testConsoleAttachmentFailureStopsTheCreatedProcessAndDisposesConsole() throws Exception { runWithHeldBarrier(Preparation.CONSOLE_ATTACHMENT_FAILURE); }
    public void testCancellationAfterProcessCreationStopsTheCreatedProcess() throws Exception { runWithHeldBarrier(Preparation.CANCEL_AFTER_PROCESS); }

    private enum Preparation { SUCCESS, LEGACY_SUCCESS, ACCEPTED_HANDOFF, HANDOFF_PROJECT_DISPOSE, SETTINGS_CHANGE, PROFILE_CHANGE, COMPILER_SWAP, IO_FAILURE, PCE, TRUST_LOSS, CANCEL, DISPOSE_ENVIRONMENT, CLOSE_PROJECT, DISPOSE_PROJECT, EDIT,
        CONSOLE_FAILURE, CONSOLE_ATTACHMENT_FAILURE, CANCEL_AFTER_PROCESS }

    private void runWithHeldBarrier(Preparation preparation) throws Exception {
        org.junit.Assume.assumeFalse("The recorder uses a POSIX executable", com.intellij.openapi.util.SystemInfo.isWindows);
        Path root = Files.createTempDirectory("nagi standard run ");
        var settings = NagiSettings.getInstance().getState();
        String previousCompiler = settings.compilerPath;
        var project = getProject();
        boolean ownedProject = preparation == Preparation.DISPOSE_PROJECT || preparation == Preparation.CLOSE_PROJECT || preparation == Preparation.HANDOFF_PROJECT_DISPOSE;
        boolean createdOwnedProject = false;
        Path ownedProjectPath = root.resolve("ide-project");
        boolean previousOwnedPathTrust = preparation == Preparation.CLOSE_PROJECT
                && TrustedProjects.isProjectTrusted(ownedProjectPath);
        boolean previousTrust = TrustedProjects.isProjectTrusted(project);
        var previousDialog = TestDialogManager.getTestImplementation();
        var barrier = new HeldBarrier(preparation);
        RunAttempt attempt = null;
        Throwable primaryFailure = null;
        try {
            if (ownedProject) {
                var manager = com.intellij.openapi.project.ex.ProjectManagerEx.getInstanceEx();
                var options = com.intellij.ide.impl.OpenProjectTask.build().asNewProject()
                        .withForceOpenInNewFrame(true).withProjectName("Nagi close regression");
                if (preparation == Preparation.CLOSE_PROJECT) {
                    // Startup checks trust before openProject returns. Trust only
                    // this owned path so the real open/close lifecycle can run.
                    TrustedProjects.setProjectTrusted(ownedProjectPath, true);
                    assertTrue(TrustedProjects.isProjectTrusted(ownedProjectPath));
                }
                var createdProject = preparation == Preparation.CLOSE_PROJECT
                        ? manager.openProject(ownedProjectPath, options)
                        : manager.newProject(ownedProjectPath, options);
                assertNotNull(createdProject);
                project = createdProject;
                createdOwnedProject = true;
                assertEquals(preparation == Preparation.CLOSE_PROJECT, manager.isProjectOpened(project));
                if (preparation == Preparation.CLOSE_PROJECT)
                    assertTrue("The owned project must already be trusted at startup", TrustedProjects.isProjectTrusted(project));
                previousTrust = TrustedProjects.isProjectTrusted(project);
            }
            Path manifest = Files.writeString(root.resolve("nagi.toml"), "entry = \"main.nagi\"\n");
            Path source = Files.writeString(root.resolve("main.nagi"), "def main():\n    print(1)\n");
            Path capturedArguments = root.resolve("arguments.txt");
            Path capturedDirectory = root.resolve("directory.txt");
            Path capturedSource = root.resolve("startup-source.nagi");
            Path compiler = root.resolve("nagic");
            // The first recorder action captures the input, then atomically publishes the observation.
            Files.writeString(compiler, "#!/bin/sh\n" + NagiRunTestCompiler.helpScript(preparation != Preparation.LEGACY_SUCCESS) + "cat '" + shellQuote(source) + "' > '"
                    + shellQuote(root.resolve("startup-source.tmp")) + "' && mv '"
                    + shellQuote(root.resolve("startup-source.tmp")) + "' '" + shellQuote(capturedSource)
                    + "' || exit 91\nprintf '%s\\n' \"$@\" > '" + shellQuote(capturedArguments)
                    + "'\npwd > '" + shellQuote(capturedDirectory) + "'\nprintf '%s' \"$NAGI_RUN_RETENTION\" > '"
                    + shellQuote(root.resolve("retention")) + "'\nexec sleep 300\n");
            assertTrue(compiler.toFile().setExecutable(true));
            settings.compilerPath = compiler.toString();
            TrustedProjects.setProjectTrusted(project, true);
            TestDialogManager.setTestDialog(message -> com.intellij.openapi.ui.TestDialog.OK.show(message));

            var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(source);
            assertNotNull(virtualFile);
            var document = FileDocumentManager.getInstance().getDocument(virtualFile);
            assertNotNull(document);
            WriteCommandAction.runWriteCommandAction(project,
                    () -> document.insertString(document.getTextLength(), "# written before run\n"));
            assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));
            String startupSource = document.getText();

            var factory = NagiRunConfigurationType.getInstance().getConfigurationFactories()[0];
            var configuration = (NagiRunConfiguration) factory.createTemplateConfiguration(project);
            configuration.setSourcePath(source.toString());
            RunContext context = runContext(configuration);
            context.environment().putUserData(NagiProgramRunner.SAVE_BARRIER, barrier);
            attempt = new RunAttempt(context, preparation, barrier);
            context.runner().execute(context.environment());
            PlatformTestUtil.waitWithEventsDispatching("Nagi runner did not reach the save barrier",
                    () -> barrier.entered.getCount() == 0, 5);
            assertFalse("Save barrier must run off EDT", barrier.ranOnEdt);
            assertFalse("Compiler started before disk writes completed", Files.exists(capturedSource));
            assertNull("No process handler may be published while saving", attempt.handler);
            assertNull("No actual process handler may be created while saving", attempt.createdHandler);
            assertNull(context.environment().getContentToReuse());
            assertFalse(FileDocumentManager.getInstance().isDocumentUnsaved(document));
            var startupResult = NagiProgramRunner.startupResult(context.environment());
            assertNotNull("A held startup must still own its result", startupResult);
            assertEquals(org.jetbrains.concurrency.Promise.State.PENDING, startupResult.getState());
            var noContentCompletion = new java.util.concurrent.atomic.AtomicBoolean();
            var completedDescriptor = new java.util.concurrent.atomic.AtomicReference<com.intellij.execution.ui.RunContentDescriptor>();
            RunAttempt ownedAttempt = attempt;
            startupResult.onSuccess(descriptor -> {
                ownedAttempt.acceptedDescriptor = descriptor;
                completedDescriptor.set(descriptor);
                noContentCompletion.set(descriptor == null);
            });

            if (preparation == Preparation.HANDOFF_PROJECT_DISPOSE) startupResult.onSuccess(descriptor -> {
                assertNotNull(descriptor);
                com.intellij.openapi.application.WriteAction.run(() -> com.intellij.openapi.util.Disposer.dispose(context.environment().getProject()));
            });
            if (preparation == Preparation.ACCEPTED_HANDOFF) startupResult.onSuccess(descriptor -> {
                assertNotNull(descriptor);
                // SDK Run-content reuse can dispose the previous descriptor's child environment
                // synchronously while the new descriptor is being accepted by this promise.
                com.intellij.openapi.util.Disposer.dispose(context.environment());
            });

            switch (preparation) {
                case SETTINGS_CHANGE -> NagiSettings.update("different-compiler", settings.checkTimeoutSeconds);
                case PROFILE_CHANGE -> configuration.setBuildProfile(NagiBuildProfile.RELEASE);
                case COMPILER_SWAP -> Files.writeString(compiler, "#!/bin/sh\nprintf unknown\n");
                case TRUST_LOSS -> TrustedProjects.setProjectTrusted(project, false);
                case CANCEL -> barrier.indicator.cancel();
                case DISPOSE_ENVIRONMENT -> com.intellij.openapi.util.Disposer.dispose(context.environment());
                case CLOSE_PROJECT -> {
                    assertTrue("The supported close operation must succeed",
                            com.intellij.openapi.project.ProjectManager.getInstance().closeAndDispose(project));
                    assertTrue("The opened project must actually be disposed", project.isDisposed());
                    assertTrue("Close must report startup cancellation before the barrier returns", attempt.notStarted);
                    assertTrue("Startup failure must be published while the project is alive", attempt.notStartedWhileProjectAlive);
                    assertTrue(attempt.failure instanceof com.intellij.openapi.progress.ProcessCanceledException);
                    assertEquals(org.jetbrains.concurrency.Promise.State.REJECTED, startupResult.getState());
                    assertNull(attempt.createdHandler);
                }
                case DISPOSE_PROJECT -> {
                    com.intellij.openapi.application.WriteAction.run(() ->
                            com.intellij.openapi.util.Disposer.dispose(context.environment().getProject()));
                    assertTrue(project.isDisposed());
                    assertEquals("Direct disposal must finish ownership without rejecting through the disposed bus",
                            org.jetbrains.concurrency.Promise.State.SUCCEEDED, startupResult.getState());
                    assertTrue("Direct disposal must have a terminal no-content outcome", noContentCompletion.get());
                    assertNull(completedDescriptor.get());
                    assertFalse("No failure may be published to the disposed project bus", attempt.notStarted);
                    assertNull(attempt.createdHandler);
                }
                case EDIT -> WriteCommandAction.runWriteCommandAction(project,
                        () -> document.insertString(document.getTextLength(), "# changed during barrier\n"));
                default -> {}
            }
            barrier.release.countDown();
            PlatformTestUtil.waitWithEventsDispatching("Save barrier did not exit", () -> barrier.exited.getCount() == 0, 5);
            // The barrier's finally precedes Runner.prepare and its EDT onSuccess.
            // Await that separate terminal phase before inspecting its outcome.
            PlatformTestUtil.waitWithEventsDispatching("Run startup did not settle after the save barrier",
                    () -> startupResult.getState() != org.jetbrains.concurrency.Promise.State.PENDING, 5);
            PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();
            PlatformTestUtil.dispatchAllEventsInIdeEventQueue();

            if (preparation == Preparation.HANDOFF_PROJECT_DISPOSE) {
                assertNotNull("Handoff must own a real process", attempt.createdHandler);
                PlatformTestUtil.waitWithEventsDispatching("Project disposal orphaned the handoff process", attempt.createdHandler::isProcessTerminated, 5);
                assertFalse(attempt.createdHandler.getProcess().isAlive());
                assertTrue(context.environment().getProject().isDisposed());
                assertEquals(org.jetbrains.concurrency.Promise.State.SUCCEEDED, startupResult.getState());
                assertFalse("No failure may be published to the disposed project bus", attempt.notStarted);
                return;
            }
            if (preparation != Preparation.SUCCESS && preparation != Preparation.ACCEPTED_HANDOFF && preparation != Preparation.LEGACY_SUCCESS) {
                assertNull("Failed/canceled startup published a handler", attempt.handler);
                if (preparation == Preparation.CONSOLE_FAILURE || preparation == Preparation.CONSOLE_ATTACHMENT_FAILURE || preparation == Preparation.CANCEL_AFTER_PROCESS) {
                    assertNotNull("The failure must happen after an actual process was created", attempt.createdHandler);
                    PlatformTestUtil.waitWithEventsDispatching("Failed startup orphaned its process", attempt.createdHandler::isProcessTerminated, 5);
                    assertFalse(attempt.createdHandler.getProcess().isAlive());
                } else {
                    assertNull("Failed/canceled startup created a handler", attempt.createdHandler);
                    assertFalse("Failed/canceled startup launched the compiler", Files.exists(capturedSource));
                }
                assertNull(context.environment().getContentToReuse());
                if (preparation == Preparation.DISPOSE_PROJECT) {
                    assertEquals("A late barrier result must not replace disposal's completed no-content outcome",
                            org.jetbrains.concurrency.Promise.State.SUCCEEDED, startupResult.getState());
                    assertTrue(noContentCompletion.get());
                    assertNull(completedDescriptor.get());
                    assertFalse(attempt.notStarted);
                } else {
                    assertEquals(org.jetbrains.concurrency.Promise.State.REJECTED, startupResult.getState());
                    assertTrue("ExecutionManager must report startup failure", attempt.notStarted);
                    if (preparation == Preparation.IO_FAILURE) {
                        assertTrue(attempt.failure instanceof ExecutionException);
                        assertTrue(attempt.failure.getCause() instanceof java.io.IOException);
                    } else if (preparation == Preparation.PCE || preparation == Preparation.CANCEL
                            || preparation == Preparation.DISPOSE_ENVIRONMENT || preparation == Preparation.CLOSE_PROJECT
                            || preparation == Preparation.CANCEL_AFTER_PROCESS) {
                        assertTrue(attempt.failure instanceof com.intellij.openapi.progress.ProcessCanceledException);
                    } else if (preparation == Preparation.CONSOLE_FAILURE || preparation == Preparation.CONSOLE_ATTACHMENT_FAILURE) {
                        assertTrue(attempt.failure instanceof IllegalStateException);
                        assertEquals(preparation == Preparation.CONSOLE_FAILURE ? "Injected console creation failure"
                                : "Injected console attachment failure", attempt.failure.getMessage());
                        if (preparation == Preparation.CONSOLE_ATTACHMENT_FAILURE) assertTrue(attempt.consoleDisposed);
                    } else {
                        assertTrue(attempt.failure instanceof ExecutionException);
                    }
                }
                if (preparation == Preparation.EDIT) assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));
                return;
            }

            RunAttempt successful = attempt;
            PlatformTestUtil.waitWithEventsDispatching("Nagi compiler did not start", () -> successful.handler != null
                    && Files.exists(capturedArguments) && Files.exists(capturedDirectory) && Files.exists(root.resolve("retention")), 5);
            var handler = (OSProcessHandler) attempt.handler;
            assertSame(attempt.createdHandler, handler);
            var descriptor = preparation == Preparation.ACCEPTED_HANDOFF ? completedDescriptor.get() : context.environment().getContentToReuse();
            assertNotNull("Standard Run content must own the actual handler", descriptor);
            assertFalse("An accepted descriptor must stay alive during environment handoff",
                    com.intellij.openapi.util.Disposer.isDisposed(descriptor));
            assertSame(handler, descriptor.getProcessHandler());
            assertTrue("Nagi compiler did not start", handler.getProcess().isAlive());
            assertEquals("Compiler must read the saved snapshot as its first action", startupSource, Files.readString(capturedSource));
            List<String> arguments = Files.readAllLines(capturedArguments);
            assertEquals(preparation == Preparation.LEGACY_SUCCESS
                    ? List.of("run", "--project", manifest.toString(), "--out", arguments.get(4))
                    : List.of("run", "--project", manifest.toString(), "--out", arguments.get(4), "--profile", "dev"), arguments);
            if (preparation == Preparation.LEGACY_SUCCESS) {
                var console = (com.intellij.execution.impl.ConsoleViewImpl) descriptor.getExecutionConsole();
                assertNotNull(console);
                console.getComponent();
                console.flushDeferredText();
                assertNotNull(console.getEditor());
                assertTrue("Fallback must identify the actual Release profile in Run output",
                        console.getEditor().getDocument().getText().contains("actual profile is Release"));
            }
            Path expectedOutputRoot = Path.of(com.intellij.openapi.application.PathManager.getSystemPath(), "nagi", "run");
            assertTrue(Path.of(arguments.get(4)).startsWith(expectedOutputRoot));
            assertEquals(root.toString(), Files.readString(capturedDirectory).strip());
            assertEquals("latest", Files.readString(root.resolve("retention")));
            assertTrue(Files.readString(source).endsWith("# written before run\n"));
            assertFalse(FileDocumentManager.getInstance().isDocumentUnsaved(document));

            com.intellij.execution.actions.StopProcessAction.stopProcess(handler);
            PlatformTestUtil.waitWithEventsDispatching("IDE Stop must terminate the compiler process", handler::isProcessTerminated, 5);
            assertFalse(handler.getProcess().isAlive());
            assertNotNull(handler.getExitCode());
        } catch (Exception | Error failure) {
            primaryFailure = failure;
            throw failure;
        } finally {
            try {
                barrier.release.countDown();
                try {
                    PlatformTestUtil.waitForAllBackgroundActivityToCalmDown();
                    if (attempt != null) attempt.close();
                } finally {
                    TestDialogManager.setTestDialog(previousDialog);
                    settings.compilerPath = previousCompiler;
                    if (!project.isDisposed()) TrustedProjects.setProjectTrusted(project, previousTrust);
                    if (preparation == Preparation.CLOSE_PROJECT)
                        TrustedProjects.setProjectTrusted(ownedProjectPath, previousOwnedPathTrust);
                }
                // The EDIT case deliberately leaves a dirty document. Save it before the final disk barrier.
                FileDocumentManager.getInstance().saveAllDocuments();
                assertEquals("Cleanup must not remove a file that the IDE still needs to save", 0,
                        FileDocumentManager.getInstance().getUnsavedDocuments().length);
                if (createdOwnedProject && !project.isDisposed()) {
                    assertTrue("Cleanup must use the supported project close path",
                            com.intellij.openapi.project.ProjectManager.getInstance().closeAndDispose(project));
                    assertTrue(project.isDisposed());
                }
                // Keep owned files if pending IDE writes were not acknowledged.
                flushBeforeDeletingSources();
                deleteTree(root);
            } catch (Exception | Error cleanupFailure) {
                if (primaryFailure == null) throw cleanupFailure;
                if (cleanupFailure != primaryFailure) primaryFailure.addSuppressed(cleanupFailure);
            }
        }
    }

    private static final class HeldBarrier implements NagiProgramRunner.SaveBarrier {
        private final java.util.concurrent.CountDownLatch entered = new java.util.concurrent.CountDownLatch(1);
        private final java.util.concurrent.CountDownLatch release = new java.util.concurrent.CountDownLatch(1);
        private final java.util.concurrent.CountDownLatch exited = new java.util.concurrent.CountDownLatch(1);
        private final Preparation preparation;
        private volatile boolean ranOnEdt;
        private volatile com.intellij.openapi.progress.ProgressIndicator indicator;

        private HeldBarrier(Preparation preparation) { this.preparation = preparation; }

        @Override public void await() throws java.io.IOException {
            ranOnEdt = com.intellij.openapi.application.ApplicationManager.getApplication().isDispatchThread();
            indicator = com.intellij.openapi.progress.ProgressManager.getInstance().getProgressIndicator();
            entered.countDown();
            try {
                if (!release.await(10, java.util.concurrent.TimeUnit.SECONDS)) throw new java.io.IOException("Test barrier was not released");
                if (preparation == Preparation.IO_FAILURE) throw new java.io.IOException("Injected save failure");
                if (preparation == Preparation.PCE) throw new com.intellij.openapi.progress.ProcessCanceledException();
                NagiSaveBarrier.awaitDiskWrites();
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                throw new java.io.IOException("Test barrier was interrupted", interrupted);
            } finally {
                exited.countDown();
            }
        }
    }

    private static final class RunAttempt implements AutoCloseable {
        private final RunContext context;
        private final com.intellij.util.messages.MessageBusConnection connection;
        private volatile com.intellij.execution.process.ProcessHandler handler;
        private volatile OSProcessHandler createdHandler;
        private volatile com.intellij.execution.ui.RunContentDescriptor acceptedDescriptor;
        private volatile boolean consoleDisposed;
        private volatile boolean notStarted;
        private volatile boolean notStartedWhileProjectAlive;
        private volatile Throwable failure;

        private RunAttempt(RunContext context, Preparation preparation, HeldBarrier barrier) {
            this.context = context;
            context.environment().putUserData(NagiRunConfiguration.PROCESS_STARTED, (state, process) -> {
                createdHandler = process;
                assertNotNull(state.getConsoleBuilder());
                if (preparation == Preparation.CANCEL_AFTER_PROCESS) barrier.indicator.cancel();
                if (preparation == Preparation.CONSOLE_FAILURE || preparation == Preparation.CONSOLE_ATTACHMENT_FAILURE) state.setConsoleBuilder(new com.intellij.execution.filters.TextConsoleBuilder() {
                    @Override public com.intellij.execution.ui.ConsoleView getConsole() {
                        if (preparation == Preparation.CONSOLE_FAILURE)
                            throw new IllegalStateException("Injected console creation failure");
                        return (com.intellij.execution.ui.ConsoleView) java.lang.reflect.Proxy.newProxyInstance(
                                com.intellij.execution.ui.ConsoleView.class.getClassLoader(),
                                new Class<?>[] {com.intellij.execution.ui.ConsoleView.class}, (proxy, method, arguments) -> {
                                    if (method.getName().equals("attachToProcess"))
                                        throw new IllegalStateException("Injected console attachment failure");
                                    if (method.getName().equals("dispose")) { consoleDisposed = true; return null; }
                                    if (method.getName().equals("hashCode")) return System.identityHashCode(proxy);
                                    if (method.getName().equals("equals")) return proxy == arguments[0];
                                    if (method.getName().equals("toString")) return "Nagi failing attachment console";
                                    throw new AssertionError("Unexpected console method: " + method.getName());
                                });
                    }
                    @Override public void addFilter(com.intellij.execution.filters.Filter filter) {}
                    @Override public void setViewer(boolean viewer) {}
                });
            });
            context.environment().setHeadless();
            connection = context.environment().getProject().getMessageBus().connect();
            connection.subscribe(com.intellij.execution.ExecutionManager.EXECUTION_TOPIC, new com.intellij.execution.ExecutionListener() {
                @Override public void processStarted(@NotNull String executorId, @NotNull ExecutionEnvironment environment,
                        @NotNull com.intellij.execution.process.ProcessHandler processHandler) {
                    if (environment == context.environment()) handler = processHandler;
                }
                @Override public void processNotStarted(@NotNull String executorId, @NotNull ExecutionEnvironment environment, Throwable cause) {
                    if (environment == context.environment()) {
                        failure = cause;
                        notStartedWhileProjectAlive = !environment.getProject().isDisposed();
                        notStarted = true;
                    }
                }
            });
        }

        @Override public void close() {
            if (createdHandler != null && !createdHandler.isProcessTerminated()) {
                if (!createdHandler.isStartNotified()) createdHandler.startNotify();
                com.intellij.execution.actions.StopProcessAction.stopProcess(createdHandler);
                PlatformTestUtil.waitWithEventsDispatching("Recorder process did not terminate during cleanup", createdHandler::isProcessTerminated, 5);
            }
            var descriptor = acceptedDescriptor != null ? acceptedDescriptor : context.environment().getContentToReuse();
            if (descriptor != null && !com.intellij.openapi.util.Disposer.isDisposed(descriptor))
                com.intellij.openapi.util.Disposer.dispose(descriptor);
            connection.disconnect();
            if (!com.intellij.openapi.util.Disposer.isDisposed(context.environment()))
                com.intellij.openapi.util.Disposer.dispose(context.environment());
        }
    }

    public void testUntrustedProjectDoesNotSaveInputsOrCreateRunState() throws Exception {
        Path root = Files.createTempDirectory("nagi untrusted standard run ");
        boolean previousTrust = TrustedProjects.isProjectTrusted(getProject());
        var previousDialog = TestDialogManager.getTestImplementation();
        try {
            Path source = Files.writeString(root.resolve("main.nagi"), "def main():\n    print(1)\n");
            var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(source);
            assertNotNull(virtualFile);
            var document = FileDocumentManager.getInstance().getDocument(virtualFile);
            assertNotNull(document);
            WriteCommandAction.runWriteCommandAction(getProject(),
                    () -> document.insertString(document.getTextLength(), "# unsaved\n"));
            assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));

            TrustedProjects.setProjectTrusted(getProject(), false);
            var messages = new java.util.ArrayList<String>();
            TestDialogManager.setTestDialog(message -> {
                messages.add(message);
                return com.intellij.openapi.ui.TestDialog.OK.show(message);
            });
            NagiRunConfiguration configuration = newConfiguration();
            configuration.setSourcePath(source.toString());
            RunContext context = runContext(configuration);

            try {
                context.runner().execute(context.environment());
                fail("An untrusted project must not create a run state");
            } catch (ExecutionException expected) {
                assertTrue(expected.getMessage().contains("Trust this project"));
            }
            assertEquals(List.of("Trust this project before executing the Nagi compiler."), messages);
            assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));
        } finally {
            TestDialogManager.setTestDialog(previousDialog);
            TrustedProjects.setProjectTrusted(getProject(), previousTrust);
            deleteTree(root);
        }
    }

    private NagiRunConfiguration newConfiguration() {
        var factories = NagiRunConfigurationType.getInstance().getConfigurationFactories();
        return (NagiRunConfiguration) factories[0].createTemplateConfiguration(getProject());
    }

    private RunContext runContext(NagiRunConfiguration configuration) {
        var executor = DefaultRunExecutor.getRunExecutorInstance();
        var runner = ProgramRunner.getRunner(executor.getId(), configuration);
        assertTrue(runner instanceof NagiProgramRunner);
        var settings = RunManager.getInstance(configuration.getProject()).createConfiguration(configuration, configuration.getFactory());
        return new RunContext(executor, runner, settings,
                new ExecutionEnvironment(executor, runner, settings, configuration.getProject()));
    }

    private static String shellQuote(Path path) {
        return path.toString().replace("'", "'\"'\"'");
    }

    private static void flushBeforeDeletingSources() throws Exception {
        var flushed = java.util.concurrent.CompletableFuture.runAsync(() -> {
            try {
                NagiSaveBarrier.awaitDiskWrites();
            } catch (java.io.IOException failure) {
                throw new java.util.concurrent.CompletionException(failure);
            }
        });
        PlatformTestUtil.waitWithEventsDispatching(
                "pending IDE file writes did not finish", flushed::isDone, 5);
        flushed.get();
    }

    private static void deleteTree(Path root) throws Exception {
        try (var paths = Files.walk(root)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
        }
    }

    private record RunContext(Executor executor, ProgramRunner<RunnerSettings> runner,
                              com.intellij.execution.RunnerAndConfigurationSettings settings,
                              ExecutionEnvironment environment) {}

}
