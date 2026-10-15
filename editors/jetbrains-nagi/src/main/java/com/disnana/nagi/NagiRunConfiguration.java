package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.Executor;
import com.intellij.execution.ExecutionResult;
import com.intellij.execution.runners.ProgramRunner;
import com.intellij.execution.configurations.ConfigurationFactory;
import com.intellij.execution.configurations.GeneralCommandLine;
import com.intellij.execution.configurations.RunConfigurationBase;
import com.intellij.execution.configurations.RunConfigurationOptions;
import com.intellij.execution.configurations.RunProfileState;
import com.intellij.execution.configurations.RuntimeConfigurationError;
import com.intellij.execution.configurations.RuntimeConfigurationException;
import com.intellij.execution.configurations.CommandLineState;
import com.intellij.execution.process.OSProcessHandler;
import com.intellij.execution.process.ProcessHandler;
import com.intellij.execution.process.ProcessTerminatedListener;
import com.intellij.execution.runners.ExecutionEnvironment;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.application.PathManager;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.ui.Messages;
import com.intellij.openapi.util.SystemInfo;
import com.intellij.openapi.util.InvalidDataException;
import java.awt.BorderLayout;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.InvalidPathException;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.HexFormat;
import javax.swing.JComponent;
import javax.swing.JLabel;
import javax.swing.JPanel;
import javax.swing.JTextField;
import org.jetbrains.annotations.NotNull;
import org.jdom.Element;
import com.intellij.openapi.options.SettingsEditor;

/** A local, standard Run configuration that delegates argument construction to NagiCommandPlan. */
public final class NagiRunConfiguration extends RunConfigurationBase<NagiRunConfigurationOptions> {
    static final com.intellij.openapi.util.Key<java.util.function.BiConsumer<CommandLineState, OSProcessHandler>> PROCESS_STARTED =
            com.intellij.openapi.util.Key.create("nagi.run.processStarted");
    public NagiRunConfiguration(@NotNull Project project, @NotNull ConfigurationFactory factory, @NotNull String name) {
        super(project, factory, name);
    }

    public String getSourcePath() { return options().getSourcePath(); }
    public void setSourcePath(String path) { options().setSourcePath(path); }

    private NagiRunConfigurationOptions options() {
        return (NagiRunConfigurationOptions) getOptions();
    }

    @Override protected Class<? extends RunConfigurationOptions> getDefaultOptionsClass() {
        return NagiRunConfigurationOptions.class;
    }

    @Override protected void doCopyOptionsFrom(RunConfigurationBase<NagiRunConfigurationOptions> other) {
        super.doCopyOptionsFrom(other);
        setSourcePath(((NagiRunConfiguration) other).getSourcePath());
    }

    @Override public void readExternal(@NotNull Element element) throws InvalidDataException {
        super.readExternal(element);
        String sourcePath = element.getAttributeValue("sourcePath");
        setSourcePath(sourcePath == null ? "" : sourcePath);
    }

    @Override public void writeExternal(@NotNull Element element) {
        super.writeExternal(element);
        if (getSourcePath().isBlank()) element.removeAttribute("sourcePath");
        else element.setAttribute("sourcePath", getSourcePath());
    }

    @Override public void checkConfiguration() throws RuntimeConfigurationException {
        sourceFile();
    }

    @Override public SettingsEditor<NagiRunConfiguration> getConfigurationEditor() {
        return new SettingsEditor<>() {
            private final JTextField sourcePath = new JTextField();

            @Override protected void resetEditorFrom(@NotNull NagiRunConfiguration configuration) {
                sourcePath.setText(configuration.getSourcePath());
            }

            @Override protected void applyEditorTo(@NotNull NagiRunConfiguration configuration) {
                configuration.setSourcePath(sourcePath.getText().strip());
            }

            @Override protected JComponent createEditor() {
                var panel = new JPanel(new BorderLayout(8, 0));
                panel.add(new JLabel("Nagi source file:"), BorderLayout.WEST);
                panel.add(sourcePath, BorderLayout.CENTER);
                return panel;
            }
        };
    }

    @Override public RunProfileState getState(@NotNull Executor executor, @NotNull ExecutionEnvironment environment)
            throws ExecutionException {
        com.intellij.openapi.application.ApplicationManager.getApplication().assertIsDispatchThread();
        Project project = getProject();
        if (project.isDisposed()) throw new ExecutionException("The Nagi project is closed.");
        Path source;
        try {
            source = sourceFile();
        } catch (RuntimeConfigurationException exception) {
            throw new ExecutionException("Select a valid Nagi source file before running.", exception);
        }
        if (!TrustedProjects.isProjectTrusted(project)) {
            Messages.showWarningDialog(project, "Trust this project before executing the Nagi compiler.", "Nagi");
            throw new ExecutionException("Trust this project before executing the Nagi compiler.");
        }

        Path workspace;
        try {
            workspace = project.getBasePath() == null ? source.getParent() : Path.of(project.getBasePath());
            if (workspace == null) workspace = source.getParent();
            var values = NagiSettings.getInstance().getState();
            Path output = outputDirectory(source);
            NagiCommandPlan plan = NagiCommandPlan.create(values.compilerPath, "run", source, workspace,
                    output, SystemInfo.isWindows);
            return new NagiCommandLineState(environment, plan);
        } catch (InvalidPathException exception) {
            throw new ExecutionException("Invalid Nagi compiler path.\n\n" + exception.getMessage(), exception);
        } catch (IllegalArgumentException exception) {
            throw new ExecutionException("Could not create the Nagi run command.\n\n" + exception.getMessage(), exception);
        }
    }

    private Path sourceFile() throws RuntimeConfigurationException {
        String configured = getSourcePath().strip();
        if (configured.isEmpty()) throw new RuntimeConfigurationError("Select a Nagi source file to run.");
        try {
            Path source = Path.of(configured).toAbsolutePath().normalize();
            String name = source.getFileName() == null ? "" : source.getFileName().toString();
            if ((!name.endsWith(".nagi") && !name.endsWith(".low")) || !Files.isRegularFile(source)) {
                throw new RuntimeConfigurationError("Select an existing .nagi or .low source file.");
            }
            return source;
        } catch (InvalidPathException exception) {
            throw new RuntimeConfigurationError("Invalid Nagi source path: " + exception.getMessage());
        }
    }

    private static Path outputDirectory(Path source) {
        Path manifest = NagiCommandPlan.findManifest(source);
        Path identifiedInput = manifest == null ? source : manifest;
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256")
                    .digest(identifiedInput.toString().getBytes(StandardCharsets.UTF_8));
            String identifier = HexFormat.of().formatHex(digest, 0, 16);
            return Path.of(PathManager.getSystemPath(), "nagi", "run", identifier);
        } catch (NoSuchAlgorithmException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static final class NagiCommandLineState extends CommandLineState {
        private final NagiCommandPlan plan;
        private OSProcessHandler startedHandler;
        private com.intellij.execution.ui.ConsoleView startedConsole;

        private NagiCommandLineState(ExecutionEnvironment environment, NagiCommandPlan plan) {
            super(environment);
            this.plan = plan;
            addConsoleFilters(new NagiDiagnosticFilter(environment.getProject(), plan.directory()));
        }

        @Override public @NotNull ExecutionResult execute(@NotNull Executor executor, @NotNull ProgramRunner<?> runner)
                throws ExecutionException {
            try {
                return super.execute(executor, runner);
            } catch (ExecutionException | RuntimeException | Error failure) {
                // CommandLineState does not clean up when console creation/attachment fails after startProcess.
                if (startedHandler != null) NagiProgramRunner.stopAfterFailedStartup(startedHandler, failure);
                if (startedConsole != null) {
                    try {
                        com.intellij.openapi.util.Disposer.dispose(startedConsole);
                    } catch (RuntimeException | Error cleanupFailure) {
                        if (cleanupFailure != failure) failure.addSuppressed(cleanupFailure);
                    }
                }
                throw failure;
            }
        }

        @Override protected com.intellij.execution.ui.ConsoleView createConsole(@NotNull Executor executor)
                throws ExecutionException {
            startedConsole = super.createConsole(executor);
            return startedConsole;
        }

        @Override protected @NotNull ProcessHandler startProcess() throws ExecutionException {
            NagiProgramRunner.checkBeforeProcessStart(getEnvironment());
            var commandLine = new GeneralCommandLine(plan.executable())
                    .withParameters(plan.arguments())
                    .withWorkDirectory(plan.directory().toFile())
                    .withCharset(StandardCharsets.UTF_8)
                    .withEnvironment("NAGI_RUN_RETENTION", "latest");
            NagiProgramRunner.checkBeforeProcessStart(getEnvironment());
            startedHandler = new OSProcessHandler(commandLine);
            startedHandler.setShouldDestroyProcessRecursively(true);
            var observer = getEnvironment().getUserData(PROCESS_STARTED);
            if (observer != null) observer.accept(this, startedHandler);
            ProcessTerminatedListener.attach(startedHandler);
            return startedHandler;
        }
    }
}
