package com.disnana.nagi;

import com.intellij.execution.configurations.RunConfigurationOptions;

/** Persistent fields for a user-created or context-produced Nagi run configuration. */
public final class NagiRunConfigurationOptions extends RunConfigurationOptions {
    private volatile String sourcePath = "";

    private volatile String buildProfile = NagiBuildProfile.DEVELOPMENT.name();

    public String getBuildProfile() { return buildProfile; }
    public void setBuildProfile(String value) { buildProfile = value == null ? NagiBuildProfile.DEVELOPMENT.name() : value; }

    public String getSourcePath() { return sourcePath; }

    public void setSourcePath(String sourcePath) {
        this.sourcePath = sourcePath == null ? "" : sourcePath;
    }
}
