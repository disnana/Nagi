package com.disnana.nagi;

/** IDE policy only. The compiler's own omitted-profile default remains Release. */
public enum NagiBuildProfile {
    DEVELOPMENT("Development"), RELEASE("Release");

    private final String label;
    NagiBuildProfile(String label) { this.label = label; }
    @Override public String toString() { return label; }
}
