package com.disnana.nagi;

import static org.junit.Assert.*;

import java.util.List;
import org.junit.Test;

public final class NagiDocumentationTest {
    private static final NagiAssistProtocol.Location LOCATION =
            new NagiAssistProtocol.Location("stdlib:example", 1, 1, 4);

    @Test public void functionFactsAreEscapedOnceWithoutRemovingTypeSyntax() {
        var definition = new NagiAssistProtocol.Definition("call", "function", LOCATION,
                "async def call(value: fn[i64, str?]) -> list[str]", List.of(
                        new NagiAssistProtocol.Parameter("<img src=x>", "map[str, list[i64?]] &lt; 😀")),
                "<script>&lt;\\u{1f600}", true, List.of());
        String html = NagiDocumentation.definition(definition);
        assertTrue(html.contains("fn[i64, str?]"));
        assertTrue(html.contains("map[str, list[i64?]] &amp;lt; 😀"));
        assertTrue(html.contains("&lt;img src=x&gt;"));
        assertTrue(html.contains("&lt;script&gt;&amp;lt;\\u{1f600}"));
        assertTrue(html.contains("Async function"));
        assertFalse(html.contains("<script>"));
    }

    @Test public void classesShowFieldsWithoutInventingAZeroArgumentConstructor() {
        var definition = new NagiAssistProtocol.Definition("Point", "class", LOCATION, "class Point\n    x: i64",
                List.of(), null, false, List.of(new NagiAssistProtocol.Parameter("x", "i64")));
        String html = NagiDocumentation.definition(definition);
        assertTrue(html.contains("Fields (use named arguments when constructing)"));
        assertTrue(html.contains("x: i64"));
        assertFalse(html.contains("Point()"));
        assertFalse(html.contains("Returns:"));
        assertFalse(html.contains("Parameters"));
    }

    @Test public void olderFactsDoNotInventReturnsOrAsyncStatus() {
        String html = NagiDocumentation.definition(new NagiAssistProtocol.Definition(
                "old", "function", LOCATION, "def old()", List.of()));
        assertFalse(html.contains("Returns:"));
        assertFalse(html.contains("Async function"));
    }

    @Test public void knownCliWrappersBecomeReadableAndKeepSourceNotesAndHints() {
        String raw = "error: line 2:12: 式が必要です (found Sym(\")\"))\n"
                + " --> /tmp/main.nagi:2\n 2 |     print(\"<tag>&lt;\")\n"
                + "note: keep this | and --> text\nhint: try a value";
        var diagnostic = new NagiAssistProtocol.Diagnostic(raw, "/tmp/main.nagi", 2, null);
        String html = NagiDocumentation.diagnosticTooltip(diagnostic);
        assertTrue(html.contains("式が必要です (found symbol: ))"));
        assertTrue(html.contains("Location: /tmp/main.nagi:2:12"));
        assertTrue(html.contains("Source:     print(&quot;&lt;tag&gt;&amp;lt;&quot;)"));
        assertTrue(html.contains("note: keep this | and --&gt; text\nhint: try a value"));
        assertFalse(html.contains("Sym("));
        assertFalse(html.contains("2 |"));
        assertEquals("the compiler message is never rewritten", raw, diagnostic.message());
    }

    @Test public void unknownFormatsOriginsAndRustEscapesArePreserved() {
        String raw = "new format: <html>&lt; `literal` 😀\\u{1f600}\n --> elsewhere:7\n 7 | text\nnote: keep";
        String html = NagiDocumentation.diagnosticTooltip(new NagiAssistProtocol.Diagnostic(raw, "/tmp/main.nagi", 2, null));
        assertTrue(html.contains(NagiDocumentation.escape(raw)));
        String escapedToken = "error: line 2: bad (found Str(\"\\u{1f600}\"))\n --> elsewhere:2\n 2 | keep";
        html = NagiDocumentation.diagnosticTooltip(new NagiAssistProtocol.Diagnostic(escapedToken, "/tmp/main.nagi", 2, null));
        assertTrue(html.contains("Str(&quot;\\u{1f600}&quot;)"));
        assertTrue(html.contains("--&gt; elsewhere:2\n 2 | keep"));
        String future = "error: line 2: bad (found FutureToken(42))\nhint: retain";
        html = NagiDocumentation.diagnosticTooltip(new NagiAssistProtocol.Diagnostic(future, null, 2, null));
        assertTrue(html.contains("FutureToken(42)"));
        assertTrue(html.contains("hint: retain"));
    }

    @Test public void htmlRendererShowsLiteralMarkupEntitiesQuotesAndUnicode() throws Exception {
        // Exercise the HTML parser/renderer's document, not just the escaped
        // HTML source: unsupported named entities can otherwise leak visibly.
        String literal = "<tag> &lt; & \"quotes\" 'apostrophe' 日本語 😀 \\u{1f600}";
        var definition = new NagiAssistProtocol.Definition("display", "function", LOCATION, literal, List.of());
        var rendered = new java.util.concurrent.atomic.AtomicReference<String>();
        javax.swing.SwingUtilities.invokeAndWait(() -> {
            var pane = new javax.swing.JEditorPane("text/html", NagiDocumentation.definition(definition));
            try { rendered.set(pane.getDocument().getText(0, pane.getDocument().getLength())); }
            catch (javax.swing.text.BadLocationException error) { throw new AssertionError(error); }
        });
        assertTrue(rendered.get(), rendered.get().contains(literal));
        assertFalse(rendered.get(), rendered.get().contains("&apos;"));
        String diagnostic = "error: line 2: " + literal + "\nhint: " + literal;
        javax.swing.SwingUtilities.invokeAndWait(() -> {
            var pane = new javax.swing.JEditorPane("text/html", NagiDocumentation.diagnosticTooltip(
                    new NagiAssistProtocol.Diagnostic(diagnostic, null, 2, null)));
            try { rendered.set(pane.getDocument().getText(0, pane.getDocument().getLength())); }
            catch (javax.swing.text.BadLocationException error) { throw new AssertionError(error); }
        });
        assertTrue(rendered.get(), rendered.get().contains(literal));
        assertTrue(rendered.get(), rendered.get().contains("hint: " + literal));
    }    @Test public void localTypesAreEscapedWithoutInventingCallableFacts() {
        String html = NagiDocumentation.local(new NagiAssistProtocol.Local("value", "list[str?] &lt; <tag> 😀", LOCATION, "local"));
        assertTrue(html.contains("value: list[str?] &amp;lt; &lt;tag&gt; 😀"));
        assertFalse(html.contains("<tag>"));
        assertFalse(html.contains("Returns:"));
        assertFalse(html.contains("Parameters"));
    }

}
