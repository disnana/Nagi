package com.disnana.nagi;

import static org.junit.Assert.*;

import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;
import org.junit.Test;

public final class NagiAssistProtocolTest {
    private static final String TEST_PATH = Path.of(System.getProperty("java.io.tmpdir"), "nagi-assist-main.nagi")
            .toAbsolutePath().normalize().toString();
    private static final String JSON_TEST_PATH = escapeJson(TEST_PATH);
    private static final String VALID = """
            {
              "format":"nagi-assist-v1",
              "semantic_status":"editor-partial",
              "frontend_checked":true,
              "frontend_accepted":false,
              "full_compile_checked":false,
              "recovered":false,
              "completion":{"kind":"names","access":"read","items":[
                {"name":"live","kind":"local","type":"bool","signature":null,
                 "target":{"file":"__TEST_PATH__","line":2,"column":5,"length":4},
                 "borrowed":false,"access":"read"}
              ]},
              "diagnostics":[{"severity":"error","stage":"check","message":"original error",
                "file":"__TEST_PATH__","line":3,"range":{"line":3,"column":5,"length":0}}],
              "symbols":{"format":"nagi-symbols-v1","files":["__TEST_PATH__"],"references":[
                {"location":{"file":"__TEST_PATH__","line":3,"column":11,"length":4},
                 "target":{"file":"__TEST_PATH__","line":2,"column":5,"length":4}}
              ]}
            }
            """.replace("__TEST_PATH__", JSON_TEST_PATH);

    @Test public void decodesOnlyCompilerCompletionAndExactNavigationFacts() {
        var response = NagiAssistProtocol.decode(VALID.getBytes(StandardCharsets.UTF_8));
        assertEquals("editor-partial", response.semanticStatus());
        assertTrue(response.frontendChecked());
        assertFalse(response.frontendAccepted());
        assertFalse(response.recovered());
        assertEquals(List.of("live"), response.completions().stream().map(NagiAssistProtocol.CompletionItem::name).toList());
        assertEquals(0, response.diagnostics().getFirst().range().length());
        assertEquals(2, response.references().getFirst().target().line());
        assertEquals(List.of(TEST_PATH), response.sourceFiles());
        assertNull("legacy expression completion has no import context", response.importContext());
    }

    @Test public void decodesSourceAndSelectedNameImportContextsWithUtf16Locations() {
        var sourceRoot = withImportContext("import-source", 1, 8, 2, List.of(Path.of(TEST_PATH).getParent().toString()), false);
        sourceRoot.getAsJsonObject("completion").getAsJsonArray("items").get(0).getAsJsonObject().addProperty("kind", "module");
        sourceRoot.getAsJsonObject("completion").getAsJsonArray("items").get(0).getAsJsonObject().addProperty("name", "std.fs");
        var source = NagiAssistProtocol.decode(sourceRoot.toString().getBytes(StandardCharsets.UTF_8));
        assertEquals("import-source", source.importContext().kind());
        assertEquals(new NagiAssistProtocol.Location(TEST_PATH, 1, 8, 2), source.importContext().replacement());
        assertEquals(List.of(Path.of(TEST_PATH).getParent().toString()), source.importContext().directories());
        assertFalse(source.importContext().incomplete());

        var namesRoot = withImportContext("import-names", 4, 12, 0, List.of(), false);
        namesRoot.getAsJsonObject("completion").getAsJsonArray("items").get(0).getAsJsonObject().addProperty("kind", "function");
        var names = NagiAssistProtocol.decode(namesRoot.toString().getBytes(StandardCharsets.UTF_8));
        assertEquals("import-names", names.importContext().kind());
        assertEquals(0, names.importContext().replacement().length());
        assertTrue(names.importContext().directories().isEmpty());
    }

    @Test public void rejectsUnknownOrContextlessImportMetadataAndOutOfParentDirectories() {
        var unknown = withImportContext("import-members", 1, 8, 2, List.of(), false);
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(unknown.toString().getBytes(StandardCharsets.UTF_8)));

        var contextless = com.google.gson.JsonParser.parseString(VALID).getAsJsonObject();
        contextless.getAsJsonObject("completion").add("replacement", location(1, 8, 2));
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(contextless.toString().getBytes(StandardCharsets.UTF_8)));

        var outside = withImportContext("import-source", 1, 8, 2, List.of(Path.of(TEST_PATH).getParent().resolve("child").toString()), false);
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(outside.toString().getBytes(StandardCharsets.UTF_8)));

        var namedWithDirectory = withImportContext("import-names", 1, 8, 0,
                List.of(Path.of(TEST_PATH).getParent().toString()), false);
        namedWithDirectory.getAsJsonObject("completion").getAsJsonArray("items").get(0).getAsJsonObject().addProperty("kind", "function");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(namedWithDirectory.toString().getBytes(StandardCharsets.UTF_8)));
    }

    @Test public void rejectsPartialImportEnumerationWhenItContainsCandidates() {
        var partial = withImportContext("import-source", 1, 8, 2, List.of(), true);
        partial.getAsJsonObject("completion").getAsJsonArray("items").get(0).getAsJsonObject().addProperty("kind", "module");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(partial.toString().getBytes(StandardCharsets.UTF_8)));

        partial.getAsJsonObject("completion").add("items", new com.google.gson.JsonArray());
        var exhausted = NagiAssistProtocol.decode(partial.toString().getBytes(StandardCharsets.UTF_8));
        assertTrue(exhausted.importContext().incomplete());
        assertTrue(exhausted.completions().isEmpty());
    }

    @Test public void rejectsFullCompileClaimsAndCandidatesFromAnEmptySemanticResult() {
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(VALID.replace("\"full_compile_checked\":false", "\"full_compile_checked\":true").getBytes(StandardCharsets.UTF_8)));
        String emptyWithCandidate = VALID.replace("\"semantic_status\":\"editor-partial\"", "\"semantic_status\":\"none\"");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(emptyWithCandidate.getBytes(StandardCharsets.UTF_8)));
        String acceptedWithoutCheck = VALID.replace("\"frontend_checked\":true", "\"frontend_checked\":false")
                .replace("\"frontend_accepted\":false", "\"frontend_accepted\":true");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(acceptedWithoutCheck.getBytes(StandardCharsets.UTF_8)));
    }

    @Test public void rejectsInventedDiagnosticSpansAndNonIntegralCoordinates() {
        String noFile = VALID.replace("\"file\":\"" + JSON_TEST_PATH + "\",\"line\":3,\"range\"", "\"line\":3,\"range\"");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(noFile.getBytes(StandardCharsets.UTF_8)));
        String fractional = VALID.replace("\"column\":5,\"length\":0", "\"column\":5.5,\"length\":0");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(fractional.getBytes(StandardCharsets.UTF_8)));
    }

    @Test public void rejectsMalformedUtf8AndOversizedOrDuplicateOverlays() {
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(new byte[] {(byte) 0xc3, (byte) 0x28}));
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.encode(new NagiAssistProtocol.Request(
                List.of(new NagiAssistProtocol.Overlay(TEST_PATH, "a"),
                        new NagiAssistProtocol.Overlay(TEST_PATH, "b")), null)));
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.encode(new NagiAssistProtocol.Request(
                java.util.Collections.nCopies(NagiAssistProtocol.MAX_FILES + 1,
                        new NagiAssistProtocol.Overlay(TEST_PATH, "")), null)));
    }

    @Test public void rejectsRelativeCompilerPathsInsteadOfResolvingAgainstTheIdeWorkingDirectory() {
        String relative = VALID.replace(JSON_TEST_PATH, "main.nagi");
        assertThrows(IllegalArgumentException.class,
                () -> NagiAssistProtocol.decode(relative.getBytes(StandardCharsets.UTF_8)));
        assertThrows(IllegalArgumentException.class,
                () -> new NagiAssistProtocol.Overlay("main.nagi", "source"));
    }

    @Test public void decodesAdditiveCompilerSignaturesAndRolesWithBoundedDetails() {
        var root = com.google.gson.JsonParser.parseString(VALID).getAsJsonObject();
        var symbols = root.getAsJsonObject("symbols");
        symbols.add("definitions", com.google.gson.JsonParser.parseString("""
                [{"name":"add","kind":"function","location":{"file":"__TEST_PATH__","line":1,"column":5,"length":3},
                  "signature":"def add(x: i64) -> i64","parameters":[{"name":"x","type":"i64"}]}]
                """.replace("__TEST_PATH__", JSON_TEST_PATH)));
        symbols.add("locals", com.google.gson.JsonParser.parseString("""
                [{"name":"x","type":"i64","kind":"parameter","location":{"file":"__TEST_PATH__","line":1,"column":9,"length":1}}]
                """.replace("__TEST_PATH__", JSON_TEST_PATH)));
        symbols.add("roles", com.google.gson.JsonParser.parseString("""
                [{"kind":"field","location":{"file":"__TEST_PATH__","line":4,"column":7,"length":5}}]
                """.replace("__TEST_PATH__", JSON_TEST_PATH)));
        var response = NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8));
        assertEquals("i64", response.definitions().getFirst().parameters().getFirst().type());
        assertEquals("parameter", response.locals().getFirst().kind());
        assertEquals("field", response.roles().getFirst().kind());
        var local = symbols.getAsJsonArray("locals").get(0).getAsJsonObject();
        local.remove("kind");
        assertEquals("local", NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)).locals().getFirst().kind());
        local.addProperty("kind", "guessed");
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)));
        local.addProperty("kind", "local");
        var definition = symbols.getAsJsonArray("definitions").get(0).getAsJsonObject();
        definition.addProperty("signature", "x".repeat(8193));
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)));
    }

    private static String escapeJson(String value) {
        return value.replace("\\", "\\\\").replace("\"", "\\\"");
    }

    @Test public void retainsOptionalDocumentationFactsAndRejectsMalformedOrOversizedFacts() {
        var root = com.google.gson.JsonParser.parseString(VALID).getAsJsonObject();
        var symbols = root.getAsJsonObject("symbols");
        symbols.add("definitions", com.google.gson.JsonParser.parseString("""
                [{"name":"add","kind":"function","location":{"file":"__TEST_PATH__","line":1,"column":5,"length":3},
                  "signature":"async def add(x: i64) -> i64","parameters":[{"name":"x","type":"i64"}],
                  "return_type":"i64","asynchronous":true,"fields":[{"name":"value","type":"list[str?]"}]}]
                """.replace("__TEST_PATH__", JSON_TEST_PATH)));
        var value = NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)).definitions().getFirst();
        assertEquals("i64", value.returnType());
        assertTrue(value.asynchronous());
        assertEquals(List.of(new NagiAssistProtocol.Parameter("value", "list[str?]")), value.fields());
        var definition = symbols.getAsJsonArray("definitions").get(0).getAsJsonObject();
        definition.addProperty("asynchronous", "true");
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)));
        definition.remove("asynchronous");
        definition.addProperty("return_type", "x".repeat(2049));
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)));
        definition.remove("return_type");
        definition.getAsJsonArray("fields").get(0).getAsJsonObject().addProperty("type", "x".repeat(2049));
        assertThrows(IllegalArgumentException.class, () -> NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)));
        definition.remove("fields");
        value = NagiAssistProtocol.decode(root.toString().getBytes(StandardCharsets.UTF_8)).definitions().getFirst();
        assertNull(value.returnType());
        assertFalse(value.asynchronous());
        assertTrue(value.fields().isEmpty());
    }

    private static com.google.gson.JsonObject withImportContext(String kind, int line, int column,
                                                                 int length, List<String> directories,
                                                                 boolean incomplete) {
        var root = com.google.gson.JsonParser.parseString(VALID).getAsJsonObject();
        var completion = root.getAsJsonObject("completion");
        completion.addProperty("context", kind);
        completion.add("replacement", location(line, column, length));
        if (!directories.isEmpty()) {
            var paths = new com.google.gson.JsonArray();
            directories.forEach(paths::add);
            completion.add("directories", paths);
        }
        if (incomplete) completion.addProperty("incomplete", true);
        return root;
    }

    private static com.google.gson.JsonObject location(int line, int column, int length) {
        var location = new com.google.gson.JsonObject();
        location.addProperty("file", JSON_TEST_PATH);
        location.addProperty("line", line);
        location.addProperty("column", column);
        location.addProperty("length", length);
        return location;
    }
}
