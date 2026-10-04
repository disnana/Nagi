package com.disnana.nagi;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

public record NagiCommandPlan(String executable, List<String> arguments, Path directory) {
    public static Path findManifest(Path source) {
        for (Path directory = source.toAbsolutePath().normalize().getParent(); directory != null; directory = directory.getParent()) {
            Path manifest = directory.resolve("nagi.toml");
            if (Files.isRegularFile(manifest)) return manifest;
        }
        return null;
    }
    public static NagiCommandPlan create(String configured, String command, Path source, Path workspace, Path cache, boolean windows) {
        if (!command.equals("check") && !command.equals("run")) throw new IllegalArgumentException("Unsupported Nagi action");
        Path manifest = findManifest(source);
        Path absoluteSource = source.toAbsolutePath().normalize();
        Path directory = manifest == null ? absoluteSource.getParent() : manifest.getParent();
        String executable = configured.isBlank() ? windows ? "nagic.exe" : "nagic" : configured.strip();
        if (!configured.isBlank() && !Path.of(executable).isAbsolute()) executable = workspace.resolve(executable).normalize().toString();
        var arguments = new ArrayList<String>();
        arguments.add(command);
        if (manifest != null) { arguments.add("--project"); arguments.add(manifest.toString()); }
        else arguments.add(absoluteSource.toString());
        arguments.add("--out");
        arguments.add(cache.toAbsolutePath().normalize().toString());
        return new NagiCommandPlan(executable, List.copyOf(arguments), directory);
    }
}
