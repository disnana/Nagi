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

/** Exposes navigation only when the compiler has an exact source-span to declaration mapping. */
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
                NagiAssistService service = file.getProject().getService(NagiAssistService.class);
                if (service == null) return PsiReference.EMPTY_ARRAY;
                NagiAssistService.CachedResponse response = service.fresh(file);
                if (response == null) {
                    service.requestNavigation(file);
                    return PsiReference.EMPTY_ARRAY;
                }
                NagiAssistProtocol.Location target = service.referenceTarget(file, element);
                if (target == null) return PsiReference.EMPTY_ARRAY;
                return new PsiReference[] { new NagiAssistReference(element, service, response, target) };
            }
        });
    }

}
