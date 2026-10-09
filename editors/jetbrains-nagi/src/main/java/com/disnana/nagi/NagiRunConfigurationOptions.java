package com.disnana.nagi;

import com.intellij.execution.configurations.RunConfigurationOptions;

/** Persistent fields for a user-created or context-produced Nagi run configuration. */
public final class NagiRunConfigurationOptions extends RunConfigurationOptions {
    private String sourcePath = "";

    public String getSourcePath() { return sourcePath; }

    public void setSourcePath(String sourcePath) {
        this.sourcePath = sourcePath == null ? "" : sourcePath;
    }
}
