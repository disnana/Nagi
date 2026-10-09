package com.disnana.nagi;

import com.intellij.patterns.PlatformPatterns;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiReference;
import com.intellij.psi.PsiReferenceBase;
import com.intellij.psi.PsiReferenceContributor;
import com.intellij.psi.PsiReferenceProvider;
import com.intellij.psi.PsiReferenceRegistrar;
import com.intellij.util.ProcessingContext;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

/** Reference targets come from the compiler's lexical/canonical binding identities. */
public final class NagiReferenceContributor extends PsiReferenceContributor {
    @Override public void registerReferenceProviders(@NotNull PsiReferenceRegistrar registrar) {
        registrar.registerReferenceProvider(PlatformPatterns.psiElement(), new PsiReferenceProvider() {
            @Override public PsiReference @NotNull [] getReferencesByElement(@NotNull PsiElement element,
                                                                            @NotNull ProcessingContext context) {
                if (element.getFirstChild() != null) return PsiReference.EMPTY_ARRAY;
                var file = element.getContainingFile();
                if (file == null || !NagiCompilerAction.isSourceFile(file.getVirtualFile())) return PsiReference.EMPTY_ARRAY;
                var service = file.getProject().getService(NagiAssistService.class);
                var snapshot = service.fresh(file);
                if (snapshot == null) {
                    service.requestNavigation(file);
                    return PsiReference.EMPTY_ARRAY;
                }
                var target = service.referenceTarget(file, element);
                if (target == null) return PsiReference.EMPTY_ARRAY;
                return new PsiReference[]{new PsiReferenceBase<PsiElement>(element, true) {
                    @Override public @Nullable PsiElement resolve() {
                        return service.resolveTarget(file, snapshot, target);
                    }
                    @Override public Object @NotNull [] getVariants() { return new Object[0]; }
                }};
            }
        });
    }
}
