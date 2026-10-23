package com.disnana.nagi;

import com.intellij.lang.parameterInfo.CreateParameterInfoContext;
import com.intellij.lang.parameterInfo.ParameterInfoHandler;
import com.intellij.lang.parameterInfo.ParameterInfoUIContext;
import com.intellij.lang.parameterInfo.UpdateParameterInfoContext;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.project.DumbAware;
import com.intellij.psi.PsiElement;
import com.intellij.psi.PsiFile;
import com.intellij.psi.TokenType;
import com.intellij.psi.tree.IElementType;
import java.util.ArrayDeque;
import java.util.Deque;

/** Bounded syntactic call context; every displayed function is resolved by the compiler. */
public final class NagiParameterInfoHandler implements ParameterInfoHandler<PsiElement, NagiAssistProtocol.Definition>, DumbAware {
    private static final int MAX_LOOKBACK = 8192;
    private static final int MAX_TOKENS = 4096;
    record Call(PsiElement opening, PsiElement callee, int parameter) {}

    static Call callAt(PsiFile file, Editor editor, int offset) {
        if (file == null || editor == null || editor.isDisposed() || offset <= 0
                || offset > editor.getDocument().getTextLength()) return null;
        var iterator = editor.getHighlighter().createIterator(offset - 1);
        Deque<IElementType> nested = new ArrayDeque<>();
        int parameter = 0;
        int visited = 0;
        var text = editor.getDocument().getCharsSequence();
        while (!iterator.atEnd()) {
            if (++visited > MAX_TOKENS || offset - iterator.getStart() > MAX_LOOKBACK) return null;
            com.intellij.openapi.progress.ProgressManager.checkCanceled();
            var token = iterator.getTokenType();
            int start = iterator.getStart();
            if (token == NagiTokens.get(NagiTokenScanner.Kind.RPAREN)) nested.push(NagiTokens.get(NagiTokenScanner.Kind.LPAREN));
            else if (token == NagiTokens.get(NagiTokenScanner.Kind.RBRACKET)) nested.push(NagiTokens.get(NagiTokenScanner.Kind.LBRACKET));
            else if (token == NagiTokens.get(NagiTokenScanner.Kind.RBRACE)) nested.push(NagiTokens.get(NagiTokenScanner.Kind.LBRACE));
            else if (token == NagiTokens.get(NagiTokenScanner.Kind.LPAREN)
                    || token == NagiTokens.get(NagiTokenScanner.Kind.LBRACKET)
                    || token == NagiTokens.get(NagiTokenScanner.Kind.LBRACE)) {
                if (!nested.isEmpty()) {
                    if (nested.pop() != token) return null;
                } else if (token == NagiTokens.get(NagiTokenScanner.Kind.LPAREN)) {
                    PsiElement opening = file.findElementAt(start);
                    iterator.retreat();
                    while (!iterator.atEnd() && iterator.getTokenType() == TokenType.WHITE_SPACE) {
                        if (++visited > MAX_TOKENS || offset - iterator.getStart() > MAX_LOOKBACK) return null;
                        iterator.retreat();
                    }
                    if (iterator.atEnd() || iterator.getTokenType() != NagiTokens.get(NagiTokenScanner.Kind.IDENTIFIER)) return null;
                    PsiElement callee = file.findElementAt(iterator.getStart());
                    // A definition's parameter list is not a call. This is a
                    // lexical context rule, not a second semantic resolver.
                    iterator.retreat();
                    while (!iterator.atEnd() && iterator.getTokenType() == TokenType.WHITE_SPACE) {
                        if (++visited > MAX_TOKENS || offset - iterator.getStart() > MAX_LOOKBACK) return null;
                        iterator.retreat();
                    }
                    if (!iterator.atEnd() && iterator.getTokenType() == NagiTokens.get(NagiTokenScanner.Kind.KEYWORD)) {
                        int length = iterator.getEnd() - iterator.getStart();
                        if (length <= 8) {
                            String previous = text.subSequence(iterator.getStart(), iterator.getEnd()).toString();
                            if (previous.equals("def") || previous.equals("fn")) return null;
                        }
                    }
                    return opening == null || callee == null ? null : new Call(opening, callee, parameter);
                } else if (token == NagiTokens.get(NagiTokenScanner.Kind.LBRACKET)) {
                    // Commas before this unmatched opener belong to its list,
                    // not to the surrounding call's current parameter.
                    parameter = 0;
                } else return null;
            } else if (nested.isEmpty() && iterator.getEnd() == start + 1) {
                char current = text.charAt(start);
                if (current == ',') parameter++;
                else if (current == ';') return null;
            }
            iterator.retreat();
        }
        return null;
    }

    @Override public PsiElement findElementForParameterInfo(CreateParameterInfoContext context) {
        Call call = callAt(context.getFile(), context.getEditor(), context.getOffset());
        if (call == null) return null;
        var service = context.getProject().getService(NagiAssistService.class);
        if (service == null) return null;
        var definition = service.functionDefinition(context.getFile(), call.callee());
        if (definition == null) {
            service.requestParameterInfo(context.getFile(), context.getEditor());
            return null;
        }
        context.setItemsToShow(new Object[] {definition});
        return call.opening();
    }

    @Override public void showParameterInfo(PsiElement element, CreateParameterInfoContext context) {
        context.showHint(element, element.getTextRange().getStartOffset(), this);
    }

    @Override public PsiElement findElementForUpdatingParameterInfo(UpdateParameterInfoContext context) {
        Call call = callAt(context.getFile(), context.getEditor(), context.getOffset());
        return call == null ? null : call.opening();
    }

    @Override public void updateParameterInfo(PsiElement element, UpdateParameterInfoContext context) {
        Call call = callAt(context.getFile(), context.getEditor(), context.getOffset());
        var service = context.getProject().getService(NagiAssistService.class);
        var definition = call == null || service == null ? null : service.functionDefinition(context.getFile(), call.callee());
        Object[] shown = context.getObjectsToView();
        if (call == null || call.opening() != element || definition == null || shown == null || shown.length != 1
                || !definition.equals(shown[0])) {
            context.removeHint();
            return;
        }
        context.setCurrentParameter(call.parameter());
    }

    @Override public void updateUI(NagiAssistProtocol.Definition definition, ParameterInfoUIContext context) {
        StringBuilder text = new StringBuilder(definition.name()).append('(');
        int start = -1;
        int end = -1;
        for (int index = 0; index < definition.parameters().size(); index++) {
            if (index > 0) text.append(", ");
            int currentStart = text.length();
            var parameter = definition.parameters().get(index);
            text.append(parameter.name()).append(": ").append(parameter.type());
            if (index == context.getCurrentParameterIndex()) { start = currentStart; end = text.length(); }
        }
        text.append(')');
        if (definition.returnType() != null) text.append(" -> ").append(definition.returnType());
        context.setupUIComponentPresentation(text.toString(), start, end, false, false, false, context.getDefaultParameterColor());
    }
}
