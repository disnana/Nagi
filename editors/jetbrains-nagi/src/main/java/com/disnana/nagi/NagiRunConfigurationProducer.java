package com.disnana.nagi;

import com.intellij.execution.actions.ConfigurationContext;
import com.intellij.execution.actions.LazyRunConfigurationProducer;
import com.intellij.execution.configurations.ConfigurationFactory;
import com.intellij.openapi.project.DumbAware;
import com.intellij.openapi.util.Ref;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import java.nio.file.Path;
import org.jetbrains.annotations.NotNull;

/** Produces a Nagi Run configuration for the active local .nagi or .low file. */
public final class NagiRunConfigurationProducer extends LazyRunConfigurationProducer<NagiRunConfiguration> implements DumbAware {
    @Override public @NotNull ConfigurationFactory getConfigurationFactory() {
        return NagiRunConfigurationType.getInstance().getConfigurationFactories()[0];
    }

    @Override protected boolean setupConfigurationFromContext(@NotNull NagiRunConfiguration configuration,
            @NotNull ConfigurationContext context, @NotNull Ref<PsiElement> sourceElement) {
        PsiElement location = context.getPsiLocation();
        PsiFile file = location == null ? null : location.getContainingFile();
        if (file == null || !NagiCompilerAction.isSourceFile(file.getVirtualFile())) return false;
        configuration.setSourcePath(Path.of(file.getVirtualFile().getPath()).toAbsolutePath().normalize().toString());
        configuration.setName("Nagi: " + file.getName());
        sourceElement.set(location);
        return true;
    }

    @Override public boolean isConfigurationFromContext(@NotNull NagiRunConfiguration configuration,
            @NotNull ConfigurationContext context) {
        PsiElement location = context.getPsiLocation();
        PsiFile file = location == null ? null : location.getContainingFile();
        if (file == null || !NagiCompilerAction.isSourceFile(file.getVirtualFile())) return false;
        String expected = Path.of(file.getVirtualFile().getPath()).toAbsolutePath().normalize().toString();
        return expected.equals(configuration.getSourcePath());
    }
}
