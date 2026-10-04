package com.disnana.nagi;

import com.intellij.execution.filters.Filter;
import com.intellij.execution.filters.OpenFileHyperlinkInfo;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.vfs.LocalFileSystem;
import java.nio.file.Path;

public final class NagiDiagnosticFilter implements Filter {
    private final Project project;
    private final Path directory;
    public NagiDiagnosticFilter(Project project, Path directory) { this.project = project; this.directory = directory; }
    @Override public Result applyFilter(String line, int entireLength) {
        var location = NagiDiagnostics.location(line);
        if (location == null) return null;
        Path path;
        try { path = NagiDiagnostics.resolve(location, directory); }
        catch (java.nio.file.InvalidPathException ignored) { return null; }
        var file = LocalFileSystem.getInstance().findFileByIoFile(path.toFile());
        if (file == null || file.isDirectory()) return null;
        int base = entireLength - line.length();
        return new Result(base + location.start(), base + location.end(),
                new OpenFileHyperlinkInfo(project, file, location.line(), location.column()));
    }
}
