package com.disnana.nagi;

import java.nio.file.Path;
import java.util.regex.Pattern;

public final class NagiDiagnostics {
    public record Location(String file, int line, int column, int start, int end) {}
    private static final Pattern LOCATION = Pattern.compile("-->\\s+(.+):(\\d+)(?::(\\d+))?\\s*$");
    private NagiDiagnostics() {}

    /** Coordinates returned to the IDE are zero-based; paths may contain a Windows drive colon. */
    public static Location location(String line) {
        var matcher = LOCATION.matcher(line.stripTrailing());
        if (!matcher.find()) return null;
        String path = matcher.group(1);
        String row = matcher.group(2);
        String column = matcher.group(3);
        // Greedy path capture must not absorb a line number when a column is present.
        var withColumn = Pattern.compile("^(.+):(\\d+)$").matcher(path);
        if (column == null && withColumn.matches()) {
            column = row;
            row = withColumn.group(2);
            path = withColumn.group(1);
        }
        try {
            return new Location(path, Math.max(0, Integer.parseInt(row) - 1), column == null ? 0 : Math.max(0, Integer.parseInt(column) - 1),
                    matcher.start(1), matcher.end());
        } catch (NumberFormatException ignored) { return null; }
    }
    public static Path resolve(Location location, Path directory) {
        Path source = Path.of(location.file());
        return source.isAbsolute() ? source.normalize() : directory.resolve(source).normalize();
    }
}
