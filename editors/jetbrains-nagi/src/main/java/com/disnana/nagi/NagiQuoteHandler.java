package com.disnana.nagi;

import com.intellij.codeInsight.editorActions.SimpleTokenSetQuoteHandler;

/** Use the IDE's quote pairing, skipping, wrapping, and paired deletion rules for Nagi strings. */
public final class NagiQuoteHandler extends SimpleTokenSetQuoteHandler {
    public NagiQuoteHandler() { super(NagiTokens.get(NagiTokenScanner.Kind.STRING)); }
}
