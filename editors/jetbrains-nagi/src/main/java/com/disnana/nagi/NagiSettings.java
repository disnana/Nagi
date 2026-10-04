package com.disnana.nagi;

import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.components.PersistentStateComponent;
import com.intellij.openapi.components.State;
import com.intellij.openapi.components.Storage;
import org.jetbrains.annotations.NotNull;

@State(name = "NagiSettings", storages = @Storage("nagi.xml"))
public final class NagiSettings implements PersistentStateComponent<NagiSettings.Values> {
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
    }
}
