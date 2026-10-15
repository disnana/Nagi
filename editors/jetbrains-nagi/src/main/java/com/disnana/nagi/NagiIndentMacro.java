package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.template.Expression;
import com.intellij.codeInsight.template.ExpressionContext;
import com.intellij.codeInsight.template.Macro;
import com.intellij.codeInsight.template.Result;
import com.intellij.codeInsight.template.TextResult;
import com.intellij.psi.PsiDocumentManager;

/** Template whitespace uses this file's effective code style, always as spaces. */
public final class NagiIndentMacro extends Macro {
    @Override public String getName() { return "nagiIndent"; }
    @Override public String getPresentableName() { return "nagiIndent()"; }
    @Override public Result calculateResult(Expression[] parameters, ExpressionContext context) {
        var editor = context.getEditor();
        if (editor == null) return null;
        var file = PsiDocumentManager.getInstance(context.getProject()).getPsiFile(editor.getDocument());
        if (file == null) return null;
        return new TextResult(" ".repeat(Math.max(0, CodeStyle.getIndentOptions(file).INDENT_SIZE)));
    }
}
