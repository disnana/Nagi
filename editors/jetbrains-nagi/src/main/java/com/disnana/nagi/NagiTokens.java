package com.disnana.nagi;

import com.intellij.psi.TokenType;
import com.intellij.lang.Language;
import com.intellij.psi.tree.IElementType;
import java.util.EnumMap;

public final class NagiTokens {
    private static final EnumMap<NagiTokenScanner.Kind, IElementType> TYPES = new EnumMap<>(NagiTokenScanner.Kind.class);
    static {
        for (var kind : NagiTokenScanner.Kind.values()) TYPES.put(kind, new IElementType(kind.name(), Language.ANY));
        TYPES.put(NagiTokenScanner.Kind.SPACE, TokenType.WHITE_SPACE);
        TYPES.put(NagiTokenScanner.Kind.BAD, TokenType.BAD_CHARACTER);
    }
    private NagiTokens() {}
    public static IElementType get(NagiTokenScanner.Kind kind) { return TYPES.get(kind); }
}
