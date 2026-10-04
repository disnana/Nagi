package com.disnana.nagi;

import com.intellij.codeInsight.daemon.LineMarkerInfo;
import com.intellij.codeInsight.daemon.LineMarkerProvider;
import com.intellij.icons.AllIcons;
import com.intellij.openapi.editor.markup.GutterIconRenderer;
import com.intellij.openapi.project.DumbAware;
import com.intellij.psi.PsiElement;
import com.intellij.psi.util.CachedValueProvider;
import com.intellij.psi.util.CachedValuesManager;
import org.jetbrains.annotations.NotNull;

/** A shortcut to the existing file/project Run action, independent of Python run configurations. */
public final class NagiRunLineMarker implements LineMarkerProvider, DumbAware {
    @Override public LineMarkerInfo<PsiElement> getLineMarkerInfo(@NotNull PsiElement element) {
        if (element.getFirstChild() != null || element.getNode().getElementType() != NagiTokens.get(NagiTokenScanner.Kind.IDENTIFIER)
                || !element.textMatches("main")) return null;
        var file = element.getContainingFile();
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return null;
        boolean low = file.getFileType() == NagiFileType.LOW;
        if (!low && file.getFileType() != NagiFileType.HIGH) return null;
        var offsets = CachedValuesManager.getCachedValue(file, () -> CachedValueProvider.Result.create(
                NagiEntryPoints.mainOffsets(file.getViewProvider().getContents(), low), file));
        if (!offsets.contains(element.getTextOffset())) return null;
        return new LineMarkerInfo<>(element, element.getTextRange(), AllIcons.Actions.Execute,
                ignored -> "Run Nagi main (nearest project or this file)", (event, target) -> {
                    if (!target.isValid()) return;
                    var targetFile = target.getContainingFile();
                    new NagiCompilerAction.Run().execute(targetFile.getProject(), targetFile.getVirtualFile());
                }, GutterIconRenderer.Alignment.LEFT, () -> "Run Nagi main");
    }
}
