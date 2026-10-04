package com.disnana.nagi;

import com.intellij.lang.Language;

public class NagiLanguage extends Language {
    public static final NagiLanguage HIGH = new High();
    public static final NagiLanguage LOW = new Low();

    private NagiLanguage(String id) { super(id); }
    private static final class High extends NagiLanguage { private High() { super("Nagi"); } }
    private static final class Low extends NagiLanguage { private Low() { super("NagiLow"); } }
}
