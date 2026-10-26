package com.disnana.nagi;

import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.components.PersistentStateComponent;
import com.intellij.openapi.components.State;
import com.intellij.openapi.components.Storage;
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer;
import com.intellij.openapi.project.ProjectManager;
import com.intellij.util.messages.Topic;
import org.jetbrains.annotations.NotNull;

@State(name = "NagiSettings", storages = @Storage("nagi.xml"))
public final class NagiSettings implements PersistentStateComponent<NagiSettings.Values> {
    public interface Listener { void settingsChanged(); }
    public static final Topic<Listener> SETTINGS_CHANGED = Topic.create("Nagi settings changed", Listener.class);
    public static final class Values {
        public String compilerPath = "";
        public int checkTimeoutSeconds = 30;
        public boolean bracketDepthColors;
    }
    private Values values = new Values();
    public static NagiSettings getInstance() { return ApplicationManager.getApplication().getService(NagiSettings.class); }
    @Override public @NotNull Values getState() { return values; }
    @Override public void loadState(@NotNull Values state) {
        boolean highlightingChanged = values.bracketDepthColors != state.bracketDepthColors;
        values = state;
        if (values.compilerPath == null) values.compilerPath = "";
        values.checkTimeoutSeconds = Math.max(1, Math.min(300, values.checkTimeoutSeconds));
        notifyChanged();
        if (highlightingChanged) restartHighlighting();
    }
    public static void update(String compilerPath, int timeoutSeconds) {
        Values current = getInstance().values;
        update(compilerPath, timeoutSeconds, current.bracketDepthColors);
    }
    public static void update(String compilerPath, int timeoutSeconds, boolean bracketDepthColors) {
        NagiSettings settings = getInstance();
        boolean highlightingChanged = settings.values.bracketDepthColors != bracketDepthColors;
        settings.values.compilerPath = compilerPath == null ? "" : compilerPath;
        settings.values.checkTimeoutSeconds = Math.max(1, Math.min(300, timeoutSeconds));
        settings.values.bracketDepthColors = bracketDepthColors;
        notifyChanged();
        if (highlightingChanged) restartHighlighting();
    }
    private static void notifyChanged() {
        ApplicationManager.getApplication().getMessageBus().syncPublisher(SETTINGS_CHANGED).settingsChanged();
    }
    private static void restartHighlighting() {
        for (var project : ProjectManager.getInstance().getOpenProjects()) {
            if (!project.isDisposed()) DaemonCodeAnalyzer.getInstance(project).restart();
        }
    }
}
