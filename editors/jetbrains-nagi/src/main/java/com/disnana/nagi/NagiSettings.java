package com.disnana.nagi;

import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.components.PersistentStateComponent;
import com.intellij.openapi.components.State;
import com.intellij.openapi.components.Storage;
import com.intellij.util.messages.Topic;
import org.jetbrains.annotations.NotNull;

@State(name = "NagiSettings", storages = @Storage("nagi.xml"))
public final class NagiSettings implements PersistentStateComponent<NagiSettings.Values> {
    public interface Listener { void settingsChanged(); }
    public static final Topic<Listener> SETTINGS_CHANGED = Topic.create("Nagi settings changed", Listener.class);
    public static final class Values {
        public String compilerPath = "";
        public int checkTimeoutSeconds = 30;
    }
    private Values values = new Values();
    public static NagiSettings getInstance() { return ApplicationManager.getApplication().getService(NagiSettings.class); }
    @Override public @NotNull Values getState() { return values; }
    @Override public void loadState(@NotNull Values state) {
        values = state;
        if (values.compilerPath == null) values.compilerPath = "";
        values.checkTimeoutSeconds = Math.max(1, Math.min(300, values.checkTimeoutSeconds));
        notifyChanged();
    }
    public static void update(String compilerPath, int timeoutSeconds) {
        NagiSettings settings = getInstance();
        settings.values.compilerPath = compilerPath == null ? "" : compilerPath;
        settings.values.checkTimeoutSeconds = Math.max(1, Math.min(300, timeoutSeconds));
        notifyChanged();
    }
    private static void notifyChanged() {
        ApplicationManager.getApplication().getMessageBus().syncPublisher(SETTINGS_CHANGED).settingsChanged();
    }
}
