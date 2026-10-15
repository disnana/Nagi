package com.disnana.nagi;

import com.intellij.codeInsight.template.TemplateActionContext;
import com.intellij.codeInsight.template.TemplateContextType;
import com.intellij.psi.PsiDocumentManager;
import org.jetbrains.annotations.NotNull;

/** Declaration snippets use lexical context only; semantic analysis stays in the compiler. */
public class NagiTemplateContext extends TemplateContextType {
    private final boolean low;

    public NagiTemplateContext() { this(false); }
    protected NagiTemplateContext(boolean low) {
        super(low ? "Nagi Low declarations" : "Nagi High declarations");
        this.low = low;
    }

    @Override public boolean isInContext(@NotNull TemplateActionContext context) {
        var file = context.getFile();
        if (file.getLanguage() != (low ? NagiLanguage.LOW : NagiLanguage.HIGH)) return false;
        var document = PsiDocumentManager.getInstance(file.getProject()).getDocument(file);
        int start = context.getStartOffset();
        if (document == null || start < 0 || start > document.getTextLength()) return false;
        int lineStart = document.getLineStartOffset(document.getLineNumber(start));
        // A function declaration starts at the top level. Do not advertise
        // Python-style nested definitions or expand inside comments/strings.
        if (start != lineStart) return false;
        var element = start < document.getTextLength() ? file.findElementAt(start) : null;
        if (element == null) return true;
        var token = element.getNode().getElementType();
        return token != NagiTokens.get(NagiTokenScanner.Kind.STRING)
                && token != NagiTokens.get(NagiTokenScanner.Kind.COMMENT);
    }

    public static final class Low extends NagiTemplateContext {
        public Low() { super(true); }
    }
}
