package com.disnana.nagi;

import com.intellij.lang.BracePair;
import com.intellij.lang.PairedBraceMatcher;
import com.intellij.psi.PsiFile;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NotNull;

public class NagiBraceMatcher implements PairedBraceMatcher {
    private final boolean low;
    public NagiBraceMatcher() { this(false); }
    protected NagiBraceMatcher(boolean low) { this.low = low; }
    private static BracePair pair(NagiTokenScanner.Kind left, NagiTokenScanner.Kind right, boolean structural) {
        return new BracePair(NagiTokens.get(left), NagiTokens.get(right), structural);
    }
    @Override public BracePair @NotNull [] getPairs() {
        var parentheses = pair(NagiTokenScanner.Kind.LPAREN, NagiTokenScanner.Kind.RPAREN, false);
        var brackets = pair(NagiTokenScanner.Kind.LBRACKET, NagiTokenScanner.Kind.RBRACKET, false);
        return low ? new BracePair[] { parentheses, brackets, pair(NagiTokenScanner.Kind.LBRACE, NagiTokenScanner.Kind.RBRACE, true) }
                : new BracePair[] { parentheses, brackets };
    }
    @Override public boolean isPairedBracesAllowedBeforeType(@NotNull IElementType left, IElementType context) { return true; }
    @Override public int getCodeConstructStart(PsiFile file, int openingBraceOffset) { return openingBraceOffset; }
    public static final class Low extends NagiBraceMatcher { public Low() { super(true); } }
}
