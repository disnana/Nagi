package com.disnana.nagi;

import com.intellij.codeInsight.completion.CompletionContributor;
import com.intellij.codeInsight.completion.CompletionParameters;
import com.intellij.codeInsight.completion.CompletionProvider;
import com.intellij.codeInsight.completion.CompletionResultSet;
import com.intellij.codeInsight.completion.CompletionType;
import com.intellij.codeInsight.completion.InsertionContext;
import com.intellij.codeInsight.completion.util.ParenthesesInsertHandler;
import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.lookup.AutoCompletionPolicy;
import com.intellij.codeInsight.lookup.LookupElement;
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

/** Declaration keywords are lexical; names and members come only from current compiler facts. */
public final class NagiAssistCompletionContributor extends CompletionContributor implements DumbAware {
    private static final int MAX_EXISTING_LIST_LOOKAHEAD = 1024;

    public NagiAssistCompletionContributor() {
        extend(CompletionType.BASIC, PlatformPatterns.psiElement(), new CompletionProvider<>() {
            @Override protected void addCompletions(@NotNull CompletionParameters parameters,
                                                    @NotNull ProcessingContext context,
                                                    @NotNull CompletionResultSet result) {
                PsiFile file = parameters.getOriginalFile();
                Editor editor = parameters.getEditor();
                if (file == null || editor == null || editor.isDisposed()
                        || (file.getLanguage() != NagiLanguage.HIGH && file.getLanguage() != NagiLanguage.LOW)
                        || !isCodeContext(file, editor.getCaretModel().getOffset())) return;
                // A partial declaration is not a compiler-checkable program. Offer
                // only the existing High/Low declaration keyword here, including
                // for a new local file or an untrusted project. The exact `def` /
                // `fn` live templates remain explicit and are never auto-expanded.
                offerDeclarationKeyword(file, editor, result);
                if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return;
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
                    if (item.kind().equals("function")) {
                        // The compiler identifies a function, not whether this use is a call or a value.
                        lookup = lookup.withInsertHandler(NagiAssistCompletionContributor::insertFunction);
                    }
                    String detail = item.type() != null ? item.type() : item.signature();
                    if (detail != null && !detail.isBlank()) lookup = lookup.withTypeText(detail, true);
                    if (item.access().equals("namespace")) lookup = lookup.withTailText(" namespace", true);
                    else if (item.borrowed()) lookup = lookup.withTailText(" borrowed read", true);
                    result.addElement(AutoCompletionPolicy.NEVER_AUTOCOMPLETE.applyPolicy(lookup));
                }
            }
        });
    }

    private static void offerDeclarationKeyword(PsiFile file, Editor editor, CompletionResultSet result) {
        String keyword = file.getLanguage() == NagiLanguage.LOW ? "fn" : "def";
        Document document = editor.getDocument();
        int offset = editor.getCaretModel().getOffset();
        if (offset < 1 || offset > document.getTextLength()) return;
        int line = document.getLineNumber(offset);
        int start = document.getLineStartOffset(line);
        int length = offset - start;
        // Only a top-level, incomplete declaration token is unambiguous. Do
        // not offer Python's nested definitions or a member named `def`/`fn`.
        if (length < 1 || length >= keyword.length()) return;
        CharSequence text = document.getCharsSequence();
        String prefix = text.subSequence(start, offset).toString();
        if (!keyword.startsWith(prefix)
                || (offset < document.getLineEndOffset(line) && !Character.isWhitespace(text.charAt(offset)))) return;
        // A column-zero token may still be inside a continuation or a Low
        // block opened on an earlier line. Unknown or over-budget is not a
        // declaration context; semantic candidates remain on their own path.
        if (!NagiDeclarationContext.isCertainTopLevel(text, offset, file.getLanguage() == NagiLanguage.LOW)) return;
        result.withPrefixMatcher(prefix).addElement(AutoCompletionPolicy.NEVER_AUTOCOMPLETE.applyPolicy(
                LookupElementBuilder.create(keyword)));
    }

    private static void insertFunction(InsertionContext context, LookupElement item) {
        if (context.getCompletionChar() != '(') return;
        int existing = existingArgumentList(context.getDocument(), context.getTailOffset());
        if (existing >= 0) {
            context.setAddCompletionChar(false);
            context.getEditor().getCaretModel().moveToOffset(existing + 1);
            return;
        }
        if (!CodeInsightSettings.getInstance().AUTOINSERT_PAIR_BRACKET) {
            context.setAddCompletionChar(false);
            int tail = context.getTailOffset();
            context.getDocument().insertString(tail, "(");
            context.getEditor().getCaretModel().moveToOffset(tail + 1);
            return;
        }
        ParenthesesInsertHandler.WITH_PARAMETERS.handleInsert(context, item);
    }

    private static int existingArgumentList(Document document, int tail) {
        CharSequence text = document.getCharsSequence();
        int limit = Math.min(text.length(), tail + MAX_EXISTING_LIST_LOOKAHEAD);
        int newlines = 0;
        for (int index = tail; index < limit; index++) {
            char current = text.charAt(index);
            if (current == '(') return index;
            if (!Character.isWhitespace(current) || (current == '\n' && ++newlines > 1)) return -1;
        }
        return -1;
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
