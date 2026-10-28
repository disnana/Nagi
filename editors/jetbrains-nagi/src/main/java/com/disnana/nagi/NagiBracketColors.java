package com.disnana.nagi;

import com.intellij.openapi.editor.DefaultLanguageHighlighterColors;
import com.intellij.openapi.editor.colors.TextAttributesKey;

import static com.intellij.openapi.editor.colors.TextAttributesKey.createTextAttributesKey;

/** Theme-owned keys: users can override every depth under Editor | Color Scheme | Nagi. */
final class NagiBracketColors {
    static final TextAttributesKey[] DEPTHS = {
            createTextAttributesKey("NAGI_BRACKET_DEPTH_1", DefaultLanguageHighlighterColors.KEYWORD),
            createTextAttributesKey("NAGI_BRACKET_DEPTH_2", DefaultLanguageHighlighterColors.STRING),
            createTextAttributesKey("NAGI_BRACKET_DEPTH_3", DefaultLanguageHighlighterColors.NUMBER),
            createTextAttributesKey("NAGI_BRACKET_DEPTH_4", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION),
            createTextAttributesKey("NAGI_BRACKET_DEPTH_5", DefaultLanguageHighlighterColors.INSTANCE_FIELD),
            createTextAttributesKey("NAGI_BRACKET_DEPTH_6", DefaultLanguageHighlighterColors.PARAMETER),
    };

    private NagiBracketColors() {}
}
