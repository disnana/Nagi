package com.disnana.nagi;

import com.intellij.patterns.PlatformPatterns;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.PsiReference;
import com.intellij.psi.PsiReferenceContributor;
import com.intellij.psi.PsiReferenceProvider;
import com.intellij.psi.PsiReferenceRegistrar;
import com.intellij.util.ProcessingContext;
import org.jetbrains.annotations.NotNull;

/** Stable syntax hosts; navigation is resolved from current exact compiler facts. */
public final class NagiAssistReferenceContributor extends PsiReferenceContributor {
    @Override public void registerReferenceProviders(@NotNull PsiReferenceRegistrar registrar) {
        registrar.registerReferenceProvider(PlatformPatterns.psiElement(), new PsiReferenceProvider() {
            @Override public PsiReference @NotNull [] getReferencesByElement(@NotNull PsiElement element,
                                                                              @NotNull ProcessingContext context) {
                PsiFile file = element.getContainingFile();
                if (file == null || !NagiCompilerAction.isSourceFile(file.getVirtualFile())
                        || !(element instanceof NagiParserDefinition.ReferenceElement)) {
                    return PsiReference.EMPTY_ARRAY;
                }
                // Platform caches this array against PSI changes only. Compiler
                // arrival must change resolution, never its syntax-only shape.
                return new PsiReference[] { new NagiAssistReference(element) };
            }
        });
    }

}
