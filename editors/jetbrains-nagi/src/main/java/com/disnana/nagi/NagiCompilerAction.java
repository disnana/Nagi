package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.configurations.GeneralCommandLine;
import com.intellij.execution.process.OSProcessHandler;
import com.intellij.execution.process.ProcessEvent;
import com.intellij.execution.process.ProcessListener;
import com.intellij.execution.process.ProcessOutputTypes;
import com.intellij.execution.process.ProcessTerminatedListener;
import com.intellij.execution.RunContentExecutor;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.actionSystem.ActionUpdateThread;
import com.intellij.openapi.actionSystem.AnAction;
import com.intellij.openapi.actionSystem.AnActionEvent;
import com.intellij.openapi.actionSystem.CommonDataKeys;
import com.intellij.openapi.application.PathManager;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.progress.ProgressIndicator;
import com.intellij.openapi.progress.Task;
import com.intellij.openapi.project.DumbAware;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.ui.Messages;
import com.intellij.openapi.util.SystemInfo;
import com.intellij.openapi.vfs.VirtualFile;
import com.intellij.util.concurrency.AppExecutorUtil;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.concurrent.TimeUnit;
import org.jetbrains.annotations.NotNull;

public abstract class NagiCompilerAction extends AnAction implements DumbAware {
    private final String command;
    protected NagiCompilerAction(String command) { this.command = command; }
    @Override public @NotNull ActionUpdateThread getActionUpdateThread() { return ActionUpdateThread.BGT; }
    @Override public void update(@NotNull AnActionEvent event) {
        var file = event.getData(CommonDataKeys.VIRTUAL_FILE);
        event.getPresentation().setEnabledAndVisible(isSourceFile(file) && event.getProject() != null);
    }
    @Override public void actionPerformed(@NotNull AnActionEvent event) {
        execute(event.getProject(), event.getData(CommonDataKeys.VIRTUAL_FILE));
    }
    final void execute(Project project, VirtualFile file) {
        if (project == null || project.isDisposed() || !isSourceFile(file)) return;
        if (!TrustedProjects.isProjectTrusted(project)) {
            Messages.showWarningDialog(project, "Trust this project before executing the Nagi compiler.", "Nagi");
            return;
        }
        if (!saveInputs()) {
            Messages.showErrorDialog(project, "One or more open files could not be saved. Save them before running the compiler.", "Nagi");
            return;
        }
        Path source = Path.of(file.getPath());
        Path workspace = project.getBasePath() == null ? source.getParent() : Path.of(project.getBasePath());
        var values = NagiSettings.getInstance().getState();
        Path manifest = NagiCommandPlan.findManifest(source);
        String identifier;
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256").digest((manifest == null ? source : manifest).toString().getBytes(StandardCharsets.UTF_8));
            identifier = HexFormat.of().formatHex(digest, 0, 16);
        } catch (java.security.NoSuchAlgorithmException impossible) { throw new IllegalStateException(impossible); }
        Path output = Path.of(PathManager.getSystemPath(), "nagi", command, identifier);
        try {
            var plan = NagiCommandPlan.create(values.compilerPath, command, source, workspace, output, SystemInfo.isWindows);
            int timeout = values.checkTimeoutSeconds;
            new Task.Backgroundable(project, "Starting Nagi " + command, true) {
                @Override public void run(@NotNull ProgressIndicator indicator) {
                    if (indicator.isCanceled() || project.isDisposed()) return;
                    try {
                        var line = new GeneralCommandLine(plan.executable()).withParameters(plan.arguments())
                                .withWorkDirectory(plan.directory().toFile()).withCharset(StandardCharsets.UTF_8);
                        var handler = new OSProcessHandler(line);
                        handler.setShouldDestroyProcessRecursively(true);
                        ProcessTerminatedListener.attach(handler);
                        if (command.equals("check")) {
                            var deadline = AppExecutorUtil.getAppScheduledExecutorService().schedule(() -> {
                                if (!handler.isProcessTerminated()) {
                                    handler.notifyTextAvailable("\nNagi check timed out. Adjust the check timeout in Nagi settings.\n", ProcessOutputTypes.STDERR);
                                    handler.destroyProcess();
                                }
                            }, timeout, TimeUnit.SECONDS);
                            handler.addProcessListener(new ProcessListener() {
                                @Override public void processTerminated(@NotNull ProcessEvent event) { deadline.cancel(false); }
                            });
                        }
                        ApplicationManager.getApplication().invokeLater(() -> {
                            if (project.isDisposed() || indicator.isCanceled()) { stopBeforeConsole(handler); return; }
                            new RunContentExecutor(project, handler).withTitle("Nagi " + command)
                                    .withFilter(new NagiDiagnosticFilter(project, plan.directory()))
                                    .withStop(handler::destroyProcess, () -> !handler.isProcessTerminated()).run();
                        });
                    } catch (ExecutionException exception) {
                        ApplicationManager.getApplication().invokeLater(() -> {
                            if (!project.isDisposed()) Messages.showErrorDialog(project,
                                    "Could not start the Nagi compiler. Check PATH or the compiler executable in Nagi settings.\n\n" + exception.getMessage(), "Nagi");
                        });
                    }
                }
            }.queue();
        } catch (java.nio.file.InvalidPathException exception) {
            Messages.showErrorDialog(project, "Invalid Nagi compiler path.\n\n" + exception.getMessage(), "Nagi");
        }
    }
    static boolean isSourceFile(VirtualFile file) {
        return file != null && file.isValid() && file.isInLocalFileSystem() && !file.isDirectory()
                && ("nagi".equals(file.getExtension()) || "low".equals(file.getExtension()));
    }
    static boolean saveInputs() {
        var documents = FileDocumentManager.getInstance();
        documents.saveAllDocuments();
        return documents.getUnsavedDocuments().length == 0;
    }
    static void stopBeforeConsole(OSProcessHandler handler) {
        // destroyProcess is deferred by the platform until startNotify. The console
        // normally starts notifications; cancelled startup has no console to do so.
        handler.startNotify();
        handler.destroyProcess();
    }
    public static final class Check extends NagiCompilerAction { public Check() { super("check"); } }
    public static final class Run extends NagiCompilerAction { public Run() { super("run"); } }
}
