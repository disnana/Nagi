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
import com.intellij.codeInsight.lookup.LookupEvent;
import com.intellij.codeInsight.lookup.LookupEx;
import com.intellij.codeInsight.lookup.LookupListener;
import com.intellij.codeInsight.lookup.LookupManager;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.util.Disposer;
import com.intellij.openapi.project.DumbAware;
import com.intellij.patterns.PlatformPatterns;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.TokenType;
import com.intellij.util.ProcessingContext;
import com.intellij.util.Alarm;
import org.jetbrains.annotations.NotNull;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

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
                NagiAssistProtocol.ImportContext importContext = current.response().importContext();
                if (importContext != null) {
                    offerImportNames(file, editor, current, service, result, importContext);
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

    /** Import spellings and replacement ranges are supplied by the compiler. */
    private static void offerImportNames(PsiFile file, Editor editor,
                                         NagiAssistService.CachedResponse current,
                                         NagiAssistService service, CompletionResultSet result,
                                         NagiAssistProtocol.ImportContext importContext) {
        if (!"names".equals(current.response().completionKind()) || importContext.incomplete()) return;
        NagiAssistProtocol.Location location = importContext.replacement();
        if (location == null || file.getVirtualFile() == null) return;
        String currentPath = file.getVirtualFile().getCanonicalPath();
        if (currentPath == null) currentPath = file.getVirtualFile().getPath();
        if (!NagiAssistService.normalizePath(location.file()).equals(NagiAssistService.normalizePath(currentPath))) return;
        Document document = editor.getDocument();
        int caret = editor.getCaretModel().getOffset();
        ImportRange range = importRange(document, location, caret);
        if (range == null) return;

        CharSequence chars = document.getCharsSequence();
        String beforeCaret = chars.subSequence(range.start(), caret).toString();
        String afterCaret = chars.subSequence(caret, range.end()).toString();
        AtomicBoolean selectionValidated = new AtomicBoolean();
        watchImportLookup(file, editor, current, service, selectionValidated);
        CompletionResultSet importResults = result.withPrefixMatcher(beforeCaret);
        for (NagiAssistProtocol.CompletionItem item : current.response().completions()) {
            com.intellij.openapi.progress.ProgressManager.checkCanceled();
            LookupElementBuilder lookup = LookupElementBuilder.create(item.name());
            String detail = item.type() != null ? item.type() : item.signature();
            if (detail != null && !detail.isBlank()) lookup = lookup.withTypeText(detail, true);
            if (item.access().equals("namespace")) lookup = lookup.withTailText(" namespace", true);
            else if (item.borrowed()) lookup = lookup.withTailText(" borrowed read", true);
            boolean importedFunctionName = importContext.kind().equals("import-names")
                    && item.kind().equals("function");
            lookup = lookup.withInsertHandler((insertion, selected) -> insertImportName(
                    insertion, item.name(), range, beforeCaret, afterCaret, importedFunctionName,
                    selectionValidated));
            importResults.addElement(AutoCompletionPolicy.NEVER_AUTOCOMPLETE.applyPolicy(lookup));
        }
    }

    /** Refuse selection if the compiler snapshot went stale while the lookup was open. */
    private static void watchImportLookup(PsiFile file, Editor editor,
                                          NagiAssistService.CachedResponse current,
                                          NagiAssistService service,
                                          AtomicBoolean selectionValidated) {
        LookupEx lookup = LookupManager.getActiveLookup(editor);
        if (lookup != null && lookup.isCompletion()) {
            attachImportLookupListener(lookup, file, editor, current, service, selectionValidated);
            return;
        }

        // Completion providers can run before the platform creates its lookup.
        // Observe that single lookup transition briefly; if it never appears,
        // the insertion handler fails closed and restores the original token.
        var scope = Disposer.newDisposable("Nagi import completion lookup guard");
        Disposer.register(file.getProject(), scope);
        Alarm watcher = new Alarm(Alarm.ThreadToUse.SWING_THREAD, scope);
        AtomicBoolean attached = new AtomicBoolean();
        long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(5000);
        Runnable poll = new Runnable() {
            @Override public void run() {
                if (attached.get()) return;
                LookupEx active = LookupManager.getActiveLookup(editor);
                if (active != null && active.getEditor() == editor && active.isCompletion()
                        && attached.compareAndSet(false, true)) {
                    attachImportLookupListener(active, file, editor, current, service, selectionValidated);
                    Disposer.dispose(scope);
                    return;
                }
                long remaining = deadline - System.nanoTime();
                if (remaining <= 0) {
                    Disposer.dispose(scope);
                    return;
                }
                int nextDelay = (int) Math.max(1, Math.min(25, TimeUnit.NANOSECONDS.toMillis(remaining)));
                watcher.addRequest(this, nextDelay);
            }
        };
        LookupEx active = LookupManager.getActiveLookup(editor);
        if (active != null && active.isCompletion() && attached.compareAndSet(false, true)) {
            attachImportLookupListener(active, file, editor, current, service, selectionValidated);
            Disposer.dispose(scope);
        } else {
            watcher.addRequest(poll, 0);
        }
    }

    private static void attachImportLookupListener(LookupEx lookup, PsiFile file, Editor editor,
                                                    NagiAssistService.CachedResponse current,
                                                    NagiAssistService service,
                                                    AtomicBoolean selectionValidated) {
        LookupListener listener = new LookupListener() {
            private void detach(LookupEvent event) {
                event.getLookup().removeLookupListener(this);
            }

            @Override public boolean beforeItemSelected(LookupEvent event) {
                if (event.getLookup() != lookup) return true;
                selectionValidated.set(false);
                if (service.fresh(file, editor) == current) {
                    selectionValidated.set(true);
                    return true;
                }
                lookup.removeLookupListener(this);
                lookup.hideLookup(true);
                return false;
            }

            @Override public void itemSelected(LookupEvent event) { detach(event); }

            @Override public void lookupCanceled(LookupEvent event) { detach(event); }
        };
        lookup.addLookupListener(listener);
    }

    private static ImportRange importRange(Document document, NagiAssistProtocol.Location location, int caret) {
        int lineIndex = location.line() - 1;
        if (lineIndex < 0 || lineIndex >= document.getLineCount()) return null;
        int lineStart = document.getLineStartOffset(lineIndex);
        int lineEnd = document.getLineEndOffset(lineIndex);
        long startLong = (long) lineStart + location.column() - 1;
        long endLong = startLong + location.length();
        if (startLong < lineStart || endLong > lineEnd || endLong > document.getTextLength()
                || startLong > caret || caret > endLong) return null;
        return new ImportRange((int) startLong, (int) endLong);
    }

    private static void insertImportName(InsertionContext context, String name, ImportRange range,
                                         String beforeCaret, String afterCaret, boolean importedFunctionName,
                                         AtomicBoolean selectionValidated) {
        if (importedFunctionName && context.getCompletionChar() == '(') context.setAddCompletionChar(false);
        Document document = context.getDocument();
        int start = context.getStartOffset();
        int tail = context.getTailOffset();
        if (!selectionValidated.getAndSet(false)) {
            restoreOriginalTarget(context, name, range, beforeCaret, afterCaret, start, tail);
            context.setAddCompletionChar(false);
            return;
        }
        CharSequence chars = document.getCharsSequence();
        int retainedPrefixLength = start - range.start();
        boolean insertedAtExpectedRange = range.start() >= 0 && range.start() <= chars.length()
                && retainedPrefixLength >= 0
                && retainedPrefixLength <= beforeCaret.length()
                && start <= tail && tail - start == name.length() && tail <= chars.length()
                && chars.subSequence(range.start(), start).toString()
                    .equals(beforeCaret.substring(0, retainedPrefixLength))
                && chars.subSequence(start, tail).toString().equals(name)
                && tail + afterCaret.length() <= chars.length()
                && chars.subSequence(tail, tail + afterCaret.length()).toString().equals(afterCaret);
        if (!insertedAtExpectedRange) {
            restoreOriginalTarget(context, name, range, beforeCaret, afterCaret, start, tail);
            return;
        }
        document.replaceString(range.start(), tail + afterCaret.length(), name);
        context.setTailOffset(range.start() + name.length());
    }

    private static void restoreOriginalTarget(InsertionContext context, String insertedName,
                                              ImportRange range, String originalPrefix,
                                              String originalSuffix, int start, int tail) {
        context.setAddCompletionChar(false);
        Document document = context.getDocument();
        CharSequence chars = document.getCharsSequence();
        int retainedPrefixLength = start - range.start();
        if (range.start() >= 0 && range.start() <= chars.length()
                && retainedPrefixLength >= 0 && retainedPrefixLength <= originalPrefix.length()
                && start <= tail && tail - start == insertedName.length()
                && tail <= chars.length()
                && chars.subSequence(range.start(), start).toString()
                    .equals(originalPrefix.substring(0, retainedPrefixLength))
                && chars.subSequence(start, tail).toString().equals(insertedName)
                && tail + originalSuffix.length() <= chars.length()
                && chars.subSequence(tail, tail + originalSuffix.length()).toString().equals(originalSuffix)) {
            document.replaceString(range.start(), tail + originalSuffix.length(), originalPrefix + originalSuffix);
            context.setTailOffset(range.start() + originalPrefix.length() + originalSuffix.length());
        }
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

    private record ImportRange(int start, int end) { }
}
