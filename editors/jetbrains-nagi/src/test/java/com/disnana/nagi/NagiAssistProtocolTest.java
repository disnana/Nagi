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

    private static String escapeJson(String value) {
        return value.replace("\\", "\\\\").replace("\"", "\\\"");
    }
}
