package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.ExecutionResult;
import com.intellij.execution.Executor;
import com.intellij.execution.RunManager;
import com.intellij.execution.configurations.RunConfiguration;
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
        org.junit.Assume.assumeFalse("The recorder uses a POSIX executable", com.intellij.openapi.util.SystemInfo.isWindows);
        Path root = Files.createTempDirectory("nagi standard run ");
        var settings = NagiSettings.getInstance().getState();
        String previousCompiler = settings.compilerPath;
        boolean previousTrust = TrustedProjects.isProjectTrusted(getProject());
        var previousDialog = TestDialogManager.getTestImplementation();
        OSProcessHandler handler = null;
        try {
            Path manifest = Files.writeString(root.resolve("nagi.toml"), "entry = \"main.nagi\"\n");
            Path source = Files.writeString(root.resolve("main.nagi"), "def main():\n    print(1)\n");
            Path capturedArguments = root.resolve("arguments.txt");
            Path capturedDirectory = root.resolve("directory.txt");
            Path compiler = root.resolve("nagic");
            Files.writeString(compiler, "#!/bin/sh\nprintf '%s\\n' \"$@\" > '" + shellQuote(capturedArguments)
                    + "'\npwd > '" + shellQuote(capturedDirectory) + "'\nexec sleep 300\n");
            assertTrue(compiler.toFile().setExecutable(true));
            settings.compilerPath = compiler.toString();
            TrustedProjects.setProjectTrusted(getProject(), true);

            var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(source);
            assertNotNull(virtualFile);
            var document = FileDocumentManager.getInstance().getDocument(virtualFile);
            assertNotNull(document);
            WriteCommandAction.runWriteCommandAction(getProject(),
                    () -> document.insertString(document.getTextLength(), "# written before run\n"));
            assertTrue(FileDocumentManager.getInstance().isDocumentUnsaved(document));

            NagiRunConfiguration configuration = newConfiguration();
            configuration.setSourcePath(source.toString());
            RunContext context = runContext(configuration);
            RunConfiguration runConfiguration = context.settings().getConfiguration();
            var state = runConfiguration.getState(context.executor(), context.environment());
            assertTrue(state instanceof com.intellij.execution.configurations.CommandLineState);
            assertNotNull(((com.intellij.execution.configurations.CommandLineState) state).getConsoleBuilder());

            ExecutionResult result = state.execute(context.executor(), context.runner());
            handler = (OSProcessHandler) result.getProcessHandler();
            handler.startNotify();
            assertTrue("Nagi compiler did not start", handler.getProcess().isAlive());
            PlatformTestUtil.waitWithEventsDispatching("Nagi compiler did not record its arguments", () ->
                    Files.exists(capturedArguments) && Files.exists(capturedDirectory), 5);

            List<String> arguments = Files.readAllLines(capturedArguments);
            assertEquals(List.of("run", "--project", manifest.toString(), "--out", arguments.get(4)), arguments);
            Path expectedOutputRoot = Path.of(com.intellij.openapi.application.PathManager.getSystemPath(), "nagi", "run");
            assertTrue(Path.of(arguments.get(4)).startsWith(expectedOutputRoot));
            assertEquals(root.toString(), Files.readString(capturedDirectory).strip());
            assertTrue(Files.readString(source).endsWith("# written before run\n"));
            assertFalse(FileDocumentManager.getInstance().isDocumentUnsaved(document));

            handler.destroyProcess();
            assertTrue("IDE Stop must terminate the compiler process", handler.waitFor(5000));
            assertFalse(handler.getProcess().isAlive());
        } finally {
            if (handler != null && !handler.isProcessTerminated()) {
                handler.startNotify();
                handler.destroyProcess();
                handler.waitFor(5000);
            }
            TestDialogManager.setTestDialog(previousDialog);
            TrustedProjects.setProjectTrusted(getProject(), previousTrust);
            settings.compilerPath = previousCompiler;
            deleteTree(root);
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
                context.settings().getConfiguration().getState(context.executor(), context.environment());
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
        var runner = new TestProgramRunner();
        var settings = RunManager.getInstance(getProject()).createConfiguration(configuration, configuration.getFactory());
        return new RunContext(executor, runner, settings,
                new ExecutionEnvironment(executor, runner, settings, getProject()));
    }

    private static String shellQuote(Path path) {
        return path.toString().replace("'", "'\"'\"'");
    }

    private static void deleteTree(Path root) throws Exception {
        try (var paths = Files.walk(root)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
        }
    }

    private record RunContext(Executor executor, ProgramRunner<RunnerSettings> runner,
                              com.intellij.execution.RunnerAndConfigurationSettings settings,
                              ExecutionEnvironment environment) {}

    private static final class TestProgramRunner implements ProgramRunner<RunnerSettings> {
        @Override public @NotNull String getRunnerId() { return "NagiTestRunner"; }
        @Override public boolean canRun(@NotNull String executorId, @NotNull com.intellij.execution.configurations.RunProfile profile) {
            return true;
        }
        @Override public void execute(@NotNull ExecutionEnvironment environment) throws ExecutionException {
            throw new UnsupportedOperationException("The test only exercises RunProfileState");
        }
    }
}
