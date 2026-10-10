package com.disnana.nagi;

import com.intellij.execution.configurations.ConfigurationFactory;
import com.intellij.execution.configurations.ConfigurationType;
import com.intellij.execution.configurations.ConfigurationTypeUtil;
import com.intellij.icons.AllIcons;
import com.intellij.openapi.components.BaseState;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.project.DumbAware;
import org.jetbrains.annotations.NotNull;

import javax.swing.Icon;

/** The standard IDE Run/Stop integration for executing a Nagi source or project. */
public final class NagiRunConfigurationType implements ConfigurationType, DumbAware {
    private final ConfigurationFactory factory = new Factory(this);

    public static @NotNull NagiRunConfigurationType getInstance() {
        return ConfigurationTypeUtil.findConfigurationType(NagiRunConfigurationType.class);
    }

    @Override public @NotNull String getDisplayName() { return "Nagi"; }
    @Override public @NotNull String getConfigurationTypeDescription() {
        return "Run a Nagi source file or the nearest Nagi project";
    }
    @Override public Icon getIcon() { return AllIcons.Actions.Execute; }
    @Override public @NotNull String getId() { return "NagiRunConfiguration"; }
    @Override public ConfigurationFactory @NotNull [] getConfigurationFactories() {
        return new ConfigurationFactory[]{factory};
    }

    public static final class Factory extends ConfigurationFactory {
        private Factory(ConfigurationType type) { super(type); }

        @Override public @NotNull String getId() { return "NagiRun"; }
        @Override public @NotNull String getName() { return "Nagi Run"; }

        @Override public @NotNull NagiRunConfiguration createTemplateConfiguration(@NotNull Project project) {
            return new NagiRunConfiguration(project, this, "Nagi Run");
        }

        @Override public @NotNull Class<? extends BaseState> getOptionsClass() {
            return NagiRunConfigurationOptions.class;
        }
    }
}
