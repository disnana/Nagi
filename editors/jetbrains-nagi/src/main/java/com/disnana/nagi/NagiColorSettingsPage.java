package com.disnana.nagi;

import com.intellij.openapi.editor.colors.TextAttributesKey;
import com.intellij.openapi.fileTypes.SyntaxHighlighter;
import com.intellij.openapi.options.colors.AttributesDescriptor;
import com.intellij.openapi.options.colors.ColorDescriptor;
import com.intellij.openapi.options.colors.ColorSettingsPage;
import java.util.LinkedHashMap;
import java.util.Map;
import javax.swing.Icon;
import org.jetbrains.annotations.NotNull;
import org.jetbrains.annotations.Nullable;

public final class NagiColorSettingsPage implements ColorSettingsPage {
    private static final AttributesDescriptor[] ATTRIBUTES = {
            descriptor(1), descriptor(2), descriptor(3), descriptor(4), descriptor(5), descriptor(6),
    };
    private static final Map<String, TextAttributesKey> TAGS = tags();

    private static AttributesDescriptor descriptor(int depth) {
        return new AttributesDescriptor("Nested brackets//Depth " + depth, NagiBracketColors.DEPTHS[depth - 1]);
    }
    private static Map<String, TextAttributesKey> tags() {
        var tags = new LinkedHashMap<String, TextAttributesKey>();
        for (int index = 0; index < NagiBracketColors.DEPTHS.length; index++) {
            tags.put("d" + (index + 1), NagiBracketColors.DEPTHS[index]);
        }
        return Map.copyOf(tags);
    }

    @Override public @Nullable Icon getIcon() { return NagiFileType.HIGH.getIcon(); }
    @Override public @NotNull SyntaxHighlighter getHighlighter() { return new NagiSyntaxHighlighter(false); }
    @Override public @NotNull String getDemoText() {
        return "def nested(value: i64):\n"
                + "    return <d1>(</d1>value + <d2>(</d2>2 * <d3>(</d3>3 + 4<d3>)</d3><d2>)</d2><d1>)</d1>\n";
    }
    @Override public @Nullable Map<String, TextAttributesKey> getAdditionalHighlightingTagToDescriptorMap() { return TAGS; }
    @Override public AttributesDescriptor @NotNull [] getAttributeDescriptors() { return ATTRIBUTES.clone(); }
    @Override public ColorDescriptor @NotNull [] getColorDescriptors() { return ColorDescriptor.EMPTY_ARRAY; }
    @Override public @NotNull String getDisplayName() { return "Nagi"; }
}
