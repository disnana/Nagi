package com.disnana.nagi;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Builds argv for the compiler-owned one-shot assist command without a shell. */
public record NagiAssistCommandPlan(String executable, List<String> arguments, Path directory) {
    public NagiAssistCommandPlan { arguments = List.copyOf(arguments); }

    public static NagiAssistCommandPlan create(String configuredCompiler, Path source,
                                               Path manifest, Path workspace, boolean windows) {
        Path absoluteSource = source.toAbsolutePath().normalize();
        Path directory = manifest == null ? absoluteSource.getParent() : manifest.toAbsolutePath().normalize().getParent();
        if (directory == null) throw new IllegalArgumentException("Nagi source has no parent directory");
        String executable = configuredCompiler == null || configuredCompiler.isBlank()
                ? windows ? "nagic.exe" : "nagic" : configuredCompiler.strip();
        if (configuredCompiler != null && !configuredCompiler.isBlank() && !Path.of(executable).isAbsolute()) {
            Path base = workspace == null ? directory : workspace;
            executable = base.resolve(executable).normalize().toString();
        }
        List<String> arguments = new ArrayList<>();
        arguments.add("assist");
        if (manifest != null) {
            // Omitting SOURCE preserves the configured entry and native replacement graph.
            arguments.add("--project");
            arguments.add(manifest.toAbsolutePath().normalize().toString());
        } else {
            arguments.add(absoluteSource.toString());
            arguments.add("--no-project");
        }
        arguments.add("--editor-input");
        return new NagiAssistCommandPlan(executable, arguments, directory);
    }
}
