package com.disnana.nagi;

import com.intellij.openapi.fileTypes.LanguageFileType;
import com.intellij.openapi.util.IconLoader;
import javax.swing.Icon;
import org.jetbrains.annotations.NotNull;

public final class NagiFileType extends LanguageFileType {
    public static final NagiFileType HIGH = new NagiFileType(false);
    public static final NagiFileType LOW = new NagiFileType(true);
    private final boolean low;

    private NagiFileType(boolean low) {
        super(low ? NagiLanguage.LOW : NagiLanguage.HIGH);
        this.low = low;
    }
    @Override public @NotNull String getName() { return low ? "Nagi Low" : "Nagi"; }
    @Override public @NotNull String getDescription() { return low ? "Nagi Low source" : "Nagi High source"; }
    @Override public @NotNull String getDefaultExtension() { return low ? "low" : "nagi"; }
    @Override public Icon getIcon() { return IconLoader.getIcon("/icons/nagi.svg", NagiFileType.class); }
}
