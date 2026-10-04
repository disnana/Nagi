package com.disnana.nagi;

import com.intellij.lexer.LexerBase;
import com.intellij.psi.tree.IElementType;
import org.jetbrains.annotations.NotNull;

public final class NagiLexer extends LexerBase {
    private final boolean low;
    private CharSequence buffer = "";
    private int end;
    private NagiTokenScanner.Token token;
    public NagiLexer(boolean low) { this.low = low; }
    @Override public void start(@NotNull CharSequence buffer, int startOffset, int endOffset, int initialState) {
        this.buffer = buffer;
        this.end = endOffset;
        token = NagiTokenScanner.next(buffer, startOffset, endOffset, low);
    }
    @Override public int getState() { return 0; }
    @Override public IElementType getTokenType() { return token == null ? null : NagiTokens.get(token.kind()); }
    @Override public int getTokenStart() { return token == null ? end : token.start(); }
    @Override public int getTokenEnd() { return token == null ? end : token.end(); }
    @Override public void advance() { if (token != null) token = NagiTokenScanner.next(buffer, token.end(), end, low); }
    @Override public @NotNull CharSequence getBufferSequence() { return buffer; }
    @Override public int getBufferEnd() { return end; }
}
