package com.disnana.nagi;

import org.junit.Test;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import static org.junit.Assert.*;

public class NagiCoreTest {
    private static List<NagiTokenScanner.Token> tokens(String source, boolean low) {
        var result = new ArrayList<NagiTokenScanner.Token>();
        for (var token = NagiTokenScanner.next(source, 0, source.length(), low); token != null;
             token = NagiTokenScanner.next(source, token.end(), source.length(), low)) {
            assertTrue("scanner must advance", token.end() > token.start());
            result.add(token);
        }
        return result;
    }
    @Test public void highAndLowKeywordsStaySeparate() {
        var high = tokens("def class fn record let", false).stream().filter(token -> token.kind() != NagiTokenScanner.Kind.SPACE).toList();
        var low = tokens("def class fn record let", true).stream().filter(token -> token.kind() != NagiTokenScanner.Kind.SPACE).toList();
        assertEquals(List.of(NagiTokenScanner.Kind.KEYWORD, NagiTokenScanner.Kind.KEYWORD, NagiTokenScanner.Kind.IDENTIFIER,
                NagiTokenScanner.Kind.IDENTIFIER, NagiTokenScanner.Kind.IDENTIFIER), high.stream().map(NagiTokenScanner.Token::kind).toList());
        assertEquals(List.of(NagiTokenScanner.Kind.IDENTIFIER, NagiTokenScanner.Kind.IDENTIFIER, NagiTokenScanner.Kind.KEYWORD,
                NagiTokenScanner.Kind.KEYWORD, NagiTokenScanner.Kind.KEYWORD), low.stream().map(NagiTokenScanner.Token::kind).toList());
    }
    @Test public void libraryNamesAreNotKeywords() {
        assertTrue(tokens("http sqlite Supervisor requests json User", false).stream()
                .allMatch(token -> token.kind() == NagiTokenScanner.Kind.IDENTIFIER || token.kind() == NagiTokenScanner.Kind.SPACE));
    }
    @Test public void stringsAndCommentsContainNoStructureTokens() {
        var scanned = tokens("print(\"{ # } \\\"quote\\\"\") # fn x() {", true);
        assertEquals(1, scanned.stream().filter(token -> token.kind() == NagiTokenScanner.Kind.STRING).count());
        assertEquals(1, scanned.stream().filter(token -> token.kind() == NagiTokenScanner.Kind.COMMENT).count());
        assertEquals(0, scanned.stream().filter(token -> token.kind() == NagiTokenScanner.Kind.LBRACE).count());
    }
    @Test public void unfinishedStringDoesNotHideNextLine() {
        var scanned = tokens("\"unfinished\nfn main() {}", true);
        assertEquals(NagiTokenScanner.Kind.STRING, scanned.getFirst().kind());
        assertTrue(scanned.stream().anyMatch(token -> token.kind() == NagiTokenScanner.Kind.KEYWORD));
    }
    @Test public void scannerCoversEntireSourceAndIncrementalOffsets() {
        String source = "fn f(x: List[i64]) -> i64 { return x[0] + 2; }\n";
        int end = 0;
        for (var token : tokens(source, true)) { assertEquals(end, token.start()); end = token.end(); }
        assertEquals(source.length(), end);
        assertEquals(3, NagiTokenScanner.next(source, 3, source.length(), true).start());
    }
    @Test public void lowFoldingKeepsStringAndCommentBracesOut() {
        String source = "fn main() {\n    print(\"}\"); # {\n    if true {\n        print(1);\n    }\n}\n";
        var folds = NagiStructure.folds(source, true);
        assertEquals(2, folds.size());
        assertEquals(source.indexOf('{') + 1, folds.getLast().start());
        assertEquals(source.lastIndexOf('}'), folds.getLast().end());
    }
    @Test public void lowFoldCannotCrossMismatchedDelimiters() {
        assertTrue(NagiStructure.folds("fn main() {\n    print(1];\n}\n", true).isEmpty());
        assertTrue(NagiStructure.folds("record Empty {};", true).isEmpty());
    }
    @Test public void highFoldingUsesActualIndentedBodies() {
        String source = "def main(): # example\n    if True:\n        print(\"not a block:\")\n    print(2)\n\ndef next():\n    print(3)\n";
        var folds = NagiStructure.folds(source, false);
        assertEquals(3, folds.size());
        assertEquals(source.indexOf(':') + 1, folds.getFirst().start());
        assertTrue(folds.getFirst().end() < source.indexOf("def next"));
    }
    @Test public void enterUsesColonOrUnclosedLowBrace() {
        assertEquals("    ", NagiStructure.indentAfter("def main():", false, 4));
        assertEquals("        ", NagiStructure.indentAfter("    case Ok(value): # }", false, 4));
        assertEquals("    ", NagiStructure.indentAfter("fn main() { # x", true, 4));
        assertEquals("", NagiStructure.indentAfter("fn empty() {}", true, 4));
    }
    @Test public void enterIgnoresPunctuationInCommentsAndStrings() {
        assertEquals("    ", NagiStructure.indentAfter("    print(\"x: { (\") # {", true, 4));
        assertEquals("    ", NagiStructure.indentAfter("    # def x():", false, 4));
        assertEquals("", NagiStructure.indentAfter("print(\"a:\")", false, 4));
    }
    @Test public void enterSupportsMultilineCallsAndCustomIndentSize() {
        assertEquals("      ", NagiStructure.indentAfter("    handler(", false, 2));
        assertEquals("        ", NagiStructure.indentAfter("    values = [", true, 4));
    }
    @Test public void completedMultilineExpressionDedentsToItsOpeningLine() {
        assertEquals("    ", NagiStructure.indentAt("def main():\n    print(\n        42)", false, 4));
        assertEquals("        ", NagiStructure.indentAt("def main():\n    values = [\n        1,", false, 4));
        assertEquals("    ", NagiStructure.indentAt("def main(\n    x: i64,\n):", false, 4));
        assertEquals("    ", NagiStructure.indentAt("fn main() {\n    print(\n        42);", true, 4));
    }
    @Test public void highFoldIgnoresIndentationInsideContinuations() {
        String source = "def main():\n    print(\n42\n)\n    print(2)\n\ndef next():\n    print(3)\n";
        var folds = NagiStructure.folds(source, false);
        assertEquals(2, folds.size());
        assertEquals(source.indexOf("print(2)") + "print(2)".length(), folds.getFirst().end());
    }
    @Test public void diagnosticsResolveUnixAndWindowsPaths() {
        var unix = NagiDiagnostics.location("  --> app/main.nagi:12\n");
        assertNotNull(unix); assertEquals("app/main.nagi", unix.file()); assertEquals(11, unix.line()); assertEquals(0, unix.column());
        var windows = NagiDiagnostics.location("  --> C:\\Users\\Example User\\main.nagi:7");
        assertNotNull(windows); assertEquals("C:\\Users\\Example User\\main.nagi", windows.file()); assertEquals(6, windows.line());
        var column = NagiDiagnostics.location("  --> C:\\demo\\file.low:9:3");
        assertNotNull(column); assertEquals("C:\\demo\\file.low", column.file()); assertEquals(8, column.line()); assertEquals(2, column.column());
        assertNull(NagiDiagnostics.location("error: invalid argument"));
        assertNull(NagiDiagnostics.location("--> main.nagi:999999999999999"));
    }
    @Test public void commandUsesNearestManifestAndPreservesArgumentBoundaries() throws Exception {
        Path root = Files.createTempDirectory("nagi plan ");
        try {
            Files.writeString(root.resolve("nagi.toml"), "entry = \"main.nagi\"\n");
            Path nested = Files.createDirectories(root.resolve("nested project"));
            Files.writeString(nested.resolve("nagi.toml"), "entry = \"main.nagi\"\n");
            Path source = nested.resolve("main.nagi");
            var plan = NagiCommandPlan.create("tool dir/nagic", "check", source, root, root.resolve("cache"), false);
            assertEquals(root.resolve("tool dir/nagic").toString(), plan.executable());
            assertEquals(List.of("check", "--project", nested.resolve("nagi.toml").toString(), "--out", root.resolve("cache").toString()), plan.arguments());
            assertEquals(nested, plan.directory());
        } finally {
            try (var paths = Files.walk(root)) {
                for (var path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }
    @Test public void looseFilesAndPathCompilerNeedNoManifest() throws Exception {
        Path root = Files.createTempDirectory("nagi loose ");
        try {
            Path source = root.resolve("a;echo secret.low");
            var plan = NagiCommandPlan.create("", "run", source, root, root.resolve("cache"), false);
            assertEquals("nagic", plan.executable());
            assertEquals(List.of("run", source.toString(), "--out", root.resolve("cache").toString()), plan.arguments());
            assertEquals("nagic.exe", NagiCommandPlan.create("", "check", source, root, root.resolve("cache"), true).executable());
            assertThrows(IllegalArgumentException.class, () -> NagiCommandPlan.create("", "shell", source, root, root, false));
        } finally { Files.delete(root); }
    }
}
