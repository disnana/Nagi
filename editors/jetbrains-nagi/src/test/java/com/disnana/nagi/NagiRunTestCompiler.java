package com.disnana.nagi;

import java.nio.charset.StandardCharsets;

/** Real CLI help fixtures; recorder side effects belong only to the subsequent run invocation. */
final class NagiRunTestCompiler {
    static String help(boolean development) throws Exception {
        try (var input = NagiRunTestCompiler.class.getResourceAsStream(development
                ? "/run-help-development.txt" : "/run-help-public-0.2.0.txt")) {
            if (input == null) throw new IllegalStateException("Missing Run help fixture");
            return new String(input.readAllBytes(), StandardCharsets.UTF_8);
        }
    }
    static String developmentHelpScript() throws Exception { return helpScript(true); }
    static String helpScript(boolean development) throws Exception {
        return "if [ \"$1\" = --help ]; then\ncat <<'NAGI_HELP_EOF'\n" + help(development) + "NAGI_HELP_EOF\nexit 0\nfi\n";
    }
    private NagiRunTestCompiler() {}
}
