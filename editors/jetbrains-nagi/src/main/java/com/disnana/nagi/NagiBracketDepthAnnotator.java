package com.disnana.nagi;

import com.intellij.lang.annotation.AnnotationHolder;
import com.intellij.lang.annotation.Annotator;
import com.intellij.lang.annotation.HighlightSeverity;
import com.intellij.openapi.progress.ProgressManager;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import org.jetbrains.annotations.NotNull;

/** Optional depth colors layered through the documented annotator API. */
public final class NagiBracketDepthAnnotator implements Annotator {
    @Override public void annotate(@NotNull PsiElement element, @NotNull AnnotationHolder holder) {
        if (!(element instanceof PsiFile file) || !NagiSettings.getInstance().getState().bracketDepthColors) return;
        boolean low = file.getLanguage() == NagiLanguage.LOW;
        if (!low && file.getLanguage() != NagiLanguage.HIGH) return;
        int index = 0;
        for (var span : NagiBracketDepth.spans(file.getViewProvider().getContents(), low)) {
            if ((index++ & 255) == 0) ProgressManager.checkCanceled();
            holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
                    .range(span.range())
                    .textAttributes(NagiBracketColors.DEPTHS[span.paletteIndex()])
                    .create();
        }
    }
}
