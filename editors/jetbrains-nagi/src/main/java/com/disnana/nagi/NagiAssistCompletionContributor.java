package com.disnana.nagi;

import com.intellij.codeInsight.completion.CompletionContributor;
import com.intellij.codeInsight.completion.CompletionParameters;
import com.intellij.codeInsight.completion.CompletionProvider;
import com.intellij.codeInsight.completion.CompletionResultSet;
import com.intellij.codeInsight.completion.CompletionType;
import com.intellij.codeInsight.lookup.AutoCompletionPolicy;
import com.intellij.codeInsight.lookup.LookupElementBuilder;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.project.DumbAware;
import com.intellij.patterns.PlatformPatterns;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.TokenType;
import com.intellij.util.ProcessingContext;
import org.jetbrains.annotations.NotNull;

/** Completion displays only candidates returned by the Nagi compiler for this exact cursor snapshot. */
public final class NagiAssistCompletionContributor extends CompletionContributor implements DumbAware {
    public NagiAssistCompletionContributor() {
        extend(CompletionType.BASIC, PlatformPatterns.psiElement(), new CompletionProvider<>() {
            @Override protected void addCompletions(@NotNull CompletionParameters parameters,
                                                    @NotNull ProcessingContext context,
                                                    @NotNull CompletionResultSet result) {
                PsiFile file = parameters.getOriginalFile();
                Editor editor = parameters.getEditor();
                if (file == null || editor == null || editor.isDisposed()
                        || !NagiCompilerAction.isSourceFile(file.getVirtualFile())
                        || !isCodeContext(file, editor.getCaretModel().getOffset())) return;
                NagiAssistService service = file.getProject().getService(NagiAssistService.class);
                if (service == null) return;
                NagiAssistService.CachedResponse current = service.fresh(file, editor);
                if (current == null) {
                    service.requestCompletion(file, editor);
                    return;
                }
                for (NagiAssistProtocol.CompletionItem item : current.response().completions()) {
                    com.intellij.openapi.progress.ProgressManager.checkCanceled();
                    LookupElementBuilder lookup = LookupElementBuilder.create(item.name());
                    String detail = item.type() != null ? item.type() : item.signature();
                    if (detail != null && !detail.isBlank()) lookup = lookup.withTypeText(detail, true);
                    if (item.access().equals("namespace")) lookup = lookup.withTailText(" namespace", true);
                    else if (item.borrowed()) lookup = lookup.withTailText(" borrowed read", true);
                    result.addElement(AutoCompletionPolicy.NEVER_AUTOCOMPLETE.applyPolicy(lookup));
                }
            }
        });
    }

    private static boolean isCodeContext(PsiFile file, int offset) {
        Document document = file.getViewProvider().getDocument();
        if (document == null || offset < 0 || offset > document.getTextLength()) return false;
        PsiElement current = offset < document.getTextLength() ? file.findElementAt(offset) : null;
        PsiElement previous = offset > 0 ? file.findElementAt(offset - 1) : null;
        if (isStringOrComment(current) || isStringOrComment(previous)) return false;
        return true;
    }

    private static boolean isStringOrComment(PsiElement element) {
        if (element == null) return false;
        var token = element.getNode().getElementType();
        return token == NagiTokens.get(NagiTokenScanner.Kind.STRING)
                || token == NagiTokens.get(NagiTokenScanner.Kind.COMMENT)
                || token == TokenType.BAD_CHARACTER;
    }
}
