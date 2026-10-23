package com.disnana.nagi;

import com.intellij.execution.ExecutionException;
import com.intellij.execution.ExecutionResult;
import com.intellij.execution.configurations.RunProfile;
import com.intellij.execution.configurations.RunProfileState;
import com.intellij.execution.configurations.RunnerSettings;
import com.intellij.execution.executors.DefaultRunExecutor;
import com.intellij.execution.process.ProcessHandler;
import com.intellij.execution.runners.AsyncProgramRunner;
import com.intellij.execution.runners.ExecutionEnvironment;
import com.intellij.execution.runners.RunContentBuilder;
import com.intellij.execution.ui.RunContentDescriptor;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.Disposable;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.progress.ProcessCanceledException;
import com.intellij.openapi.progress.ProgressIndicator;
import com.intellij.openapi.progress.Task;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.project.ProjectManager;
import com.intellij.openapi.project.ProjectManagerListener;
import com.intellij.openapi.util.Disposer;
import com.intellij.openapi.util.Key;
import java.io.IOException;
import java.util.concurrent.atomic.AtomicBoolean;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.concurrency.AsyncPromise;
import org.jetbrains.concurrency.Promise;

/** Keeps save-to-disk waits off EDT while retaining the platform's normal Run/Stop ownership. */
public final class NagiProgramRunner extends AsyncProgramRunner<RunnerSettings> {
    private static final Key<Startup> STARTUP = Key.create("nagi.run.startup");
    // Per-execution seam: tests exercise the registered runner without a shared/global override.
    static final Key<SaveBarrier> SAVE_BARRIER = Key.create("nagi.run.saveBarrier");

    @FunctionalInterface
    interface SaveBarrier {
        void await() throws IOException;
    }

    @Override public @NotNull String getRunnerId() { return "NagiRunRunner"; }

    @Override public boolean canRun(@NotNull String executorId, @NotNull RunProfile profile) {
        return DefaultRunExecutor.EXECUTOR_ID.equals(executorId) && profile instanceof NagiRunConfiguration;
    }

    @Override protected @NotNull Promise<RunContentDescriptor> execute(
            @NotNull ExecutionEnvironment environment, @NotNull RunProfileState state) throws ExecutionException {
        ApplicationManager.getApplication().assertIsDispatchThread();
        checkProject(environment);
        if (!NagiCompilerAction.saveInputs()) {
            throw new ExecutionException("One or more open files could not be saved. Save them before running the compiler.");
        }
        var startup = new Startup(environment);
        environment.putUserData(STARTUP, startup);
        try {
            startup.attach();
            new Task.Backgroundable(environment.getProject(), "Preparing Nagi Run", true) {
                private IOException saveFailure;
                private ExecutionException preparationFailure;
                // Task.queue otherwise runs headless/unit-test tasks synchronously, including on EDT.
                @Override public boolean isHeadless() { return false; }

                @Override public void run(@NotNull ProgressIndicator indicator) {
                    startup.indicator = indicator;
                    startup.checkCanceled();
                    ApplicationManager.getApplication().assertIsNonDispatchThread();
                    try {
                        SaveBarrier barrier = environment.getUserData(SAVE_BARRIER);
                        if (barrier == null) NagiSaveBarrier.awaitDiskWrites();
                        else barrier.await();
                    } catch (IOException failure) {
                        saveFailure = failure;
                        return;
                    }
                    startup.checkCanceled();
                    try {
                        if (!(state instanceof NagiRunConfiguration.NagiCommandLineState nagiState))
                            throw new ExecutionException("Unexpected Nagi Run state.");
                        nagiState.prepare(indicator, () -> { startup.checkCanceled(); return true; });
                    } catch (ExecutionException failure) { preparationFailure = failure; }
                    startup.checkCanceled();
                }

                @Override public void onSuccess() {
                    if (saveFailure != null) startup.fail(new ExecutionException(
                            "Could not save files to disk before running the Nagi compiler.", saveFailure));
                    else if (preparationFailure != null) startup.fail(preparationFailure);
                    else startup.launch(state);
                }
                @Override public void onCancel() { startup.fail(new ProcessCanceledException()); }
                @Override public void onThrowable(@NotNull Throwable failure) { startup.fail(failure); }
            }.queue();
        } catch (RuntimeException | Error failure) {
            startup.fail(failure);
        }
        return startup.result;
    }

    // Read-only observation used by the actual-runner lifecycle tests.
    static Promise<RunContentDescriptor> startupResult(ExecutionEnvironment environment) {
        Startup startup = environment.getUserData(STARTUP);
        return startup == null ? null : startup.result;
    }

    static void checkBeforeProcessStart(ExecutionEnvironment environment) throws ExecutionException {
        Startup startup = environment.getUserData(STARTUP);
        if (startup == null) throw new ProcessCanceledException();
        startup.checkBeforeLaunch();
    }

    private static void checkProject(ExecutionEnvironment environment) throws ExecutionException {
        if (environment.getProject().isDisposed() || Disposer.isDisposed(environment)) throw new ProcessCanceledException();
        try {
            if (!TrustedProjects.isProjectTrusted(environment.getProject()))
                throw new ExecutionException("Trust this project before executing the Nagi compiler.");
        } catch (ProcessCanceledException canceled) {
            throw canceled;
        } catch (RuntimeException | LinkageError failure) {
            throw new ExecutionException("Project trust could not be confirmed; compiler execution was refused.", failure);
        }
    }

    static void stopAfterFailedStartup(ProcessHandler handler, Throwable primary) {
        try {
            // The platform defers destroyProcess until startNotify, normally supplied by Run content.
            if (!handler.isStartNotified()) handler.startNotify();
        } catch (RuntimeException | Error cleanupFailure) {
            if (cleanupFailure != primary) primary.addSuppressed(cleanupFailure);
        }
        try {
            handler.destroyProcess();
        } catch (RuntimeException | Error cleanupFailure) {
            if (cleanupFailure != primary) primary.addSuppressed(cleanupFailure);
        }
    }

    private static final class Startup {
        private final ExecutionEnvironment environment;
        private final AsyncPromise<RunContentDescriptor> result = new AsyncPromise<>();
        private enum Ownership { PREPARING, ABORTED, HANDED_OFF }
        private final java.util.concurrent.atomic.AtomicReference<Ownership> ownership =
                new java.util.concurrent.atomic.AtomicReference<>(Ownership.PREPARING);
        private final AtomicBoolean retired = new AtomicBoolean();
        private final java.util.concurrent.atomic.AtomicReference<ProcessHandler> handoffProcess = new java.util.concurrent.atomic.AtomicReference<>();
        private final AtomicBoolean canceled = new AtomicBoolean();
        // A disposal fallback prevents late launch, but a parent's beforeTreeDispose may already
        // mark the project disposed. The supported projectClosing notification below settles earlier.
        private final Disposable projectLifetime = new Disposable.Parent() {
            @Override public void beforeTreeDispose() { projectClosing(); }
            @Override public void dispose() { projectClosing(); }
        };
        private final Disposable environmentLifetime = new Disposable.Parent() {
            @Override public void beforeTreeDispose() { cancel(); }
            @Override public void dispose() { cancel(); }
        };
        private volatile ProgressIndicator indicator;

        private Startup(ExecutionEnvironment environment) {
            this.environment = environment;
            // Keep failure ownership with ExecutionManager, including failures before it attaches its listener.
            result.onError(ignored -> {});
            result.onProcessed(ignored -> {
                // setResult runs SDK success callbacks inline on EDT. Keep the project-only
                // process fallback until those callbacks return, regardless of callback order.
                if (ownership.get() != Ownership.HANDED_OFF) retire();
            });
        }

        private void attach() {
            Disposer.register(environment.getProject(), projectLifetime);
            Disposer.register(environment, environmentLifetime);
            ApplicationManager.getApplication().getMessageBus().connect(projectLifetime).subscribe(
                    ProjectManager.TOPIC, new ProjectManagerListener() {
                        @Override public void projectClosing(@NotNull Project project) {
                            if (project == environment.getProject()) Startup.this.projectClosing();
                        }
                    });
        }

        private void cancel() {
            fail(new ProcessCanceledException());
        }

        private void fail(Throwable failure) {
            if (!ownership.compareAndSet(Ownership.PREPARING, Ownership.ABORTED)) return;
            canceled.set(true);
            ProgressIndicator current = indicator;
            if (current != null) current.cancel();
            if (environment.getProject().isDisposed()) {
                // Abnormal/direct disposal can bypass projectClosing. A null descriptor is the
                // platform's terminal no-content outcome: its success continuation expires with
                // the project, whereas rejecting here would publish to an already disposed bus.
                // This completes ownership; later task completion cannot replace this outcome.
                canceled.set(true);
                result.setResult(null);
            } else {
                result.setError(failure);
            }
        }

        private void projectClosing() {
            cancel();
            // Environment disposal during accepted Run-content reuse is benign. Project
            // closure is different: SDK success dispatch may expire before it owns Stop.
            ProcessHandler handler = handoffProcess.get();
            if (handler != null) stopAfterFailedStartup(handler, new ProcessCanceledException());
        }

        private void retire() {
            if (!retired.compareAndSet(false, true)) return;
            if (!Disposer.isDisposed(projectLifetime)) Disposer.dispose(projectLifetime);
            if (!Disposer.isDisposed(environmentLifetime)) Disposer.dispose(environmentLifetime);
            if (environment.getUserData(STARTUP) == this) environment.putUserData(STARTUP, null);
        }

        private void checkCanceled() {
            if (canceled.get() || result.isDone() || environment.getProject().isDisposed()
                    || Disposer.isDisposed(environment)) throw new ProcessCanceledException();
            ProgressIndicator current = indicator;
            if (current != null) current.checkCanceled();
        }

        private void checkBeforeLaunch() throws ExecutionException {
            ApplicationManager.getApplication().assertIsDispatchThread();
            checkCanceled();
            checkProject(environment);
            // Edits during the flush require a new Run; do not resave here and launch unflushed bytes.
            if (FileDocumentManager.getInstance().getUnsavedDocuments().length != 0)
                throw new ExecutionException("Files changed while preparing Nagi Run. Save them and run again.");
        }

        private void launch(RunProfileState state) {
            ExecutionResult execution = null;
            RunContentBuilder builder = null;
            RunContentDescriptor descriptor = null;
            try {
                checkBeforeLaunch();
                execution = state.execute(environment.getExecutor(), environment.getRunner());
                checkBeforeLaunch();
                builder = new RunContentBuilder(execution, environment);
                descriptor = builder.showRunContent(environment.getContentToReuse());
                checkBeforeLaunch();
                handoffProcess.set(execution.getProcessHandler());
                if (!ownership.compareAndSet(Ownership.PREPARING, Ownership.HANDED_OFF)) throw new ProcessCanceledException();
                // This CAS owns the only terminal success write. Reentrant disposal of the
                // old content's environment cannot revoke the new descriptor's ownership.
                try { result.setResult(descriptor); }
                finally {
                    if (environment.getProject().isDisposed()) projectClosing();
                    handoffProcess.set(null);
                    retire();
                }
            } catch (ExecutionException | RuntimeException | Error failure) {
                if (ownership.get() == Ownership.HANDED_OFF) {
                    if (failure instanceof RuntimeException runtime) throw runtime;
                    if (failure instanceof Error error) throw error;
                    throw new IllegalStateException("Run-content acceptance failed after ownership transfer.", failure);
                }
                handoffProcess.set(null);
                if (execution != null) stopAfterFailedStartup(execution.getProcessHandler(), failure);
                try {
                    if (descriptor != null) Disposer.dispose(descriptor);
                    else if (builder != null) Disposer.dispose(builder);
                    else if (execution != null && execution.getExecutionConsole() != null)
                        Disposer.dispose(execution.getExecutionConsole());
                } catch (RuntimeException | Error cleanupFailure) {
                    if (cleanupFailure != failure) failure.addSuppressed(cleanupFailure);
                }
                fail(failure);
            }
        }
    }
}
