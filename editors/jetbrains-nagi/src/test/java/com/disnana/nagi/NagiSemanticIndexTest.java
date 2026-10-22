package com.disnana.nagi;

import static org.junit.Assert.*;

import com.intellij.openapi.editor.DefaultLanguageHighlighterColors;
import java.nio.file.Path;
import java.util.List;
import org.junit.Test;

public final class NagiSemanticIndexTest {
    private static final String FILE = Path.of(System.getProperty("java.io.tmpdir"), "semantic-main.nagi").toAbsolutePath().toString();
    private static NagiAssistProtocol.Location at(int line, int column, int length) {
        return new NagiAssistProtocol.Location(FILE, line, column, length);
    }
    private static NagiAssistProtocol.Response facts(String status, List<NagiAssistProtocol.Definition> definitions,
                                                    List<NagiAssistProtocol.Local> locals, List<NagiAssistProtocol.Reference> references) {
        return new NagiAssistProtocol.Response(status, true, true, false, "names", List.of(), List.of(),
                references, List.of(FILE), definitions, locals);
    }
    @Test public void exactTargetsDistinguishSameNameAndParameterRoles() {
        var function = new NagiAssistProtocol.Definition("value", "function", at(1, 5, 5), "def value(x: i64) -> i64",
                List.of(new NagiAssistProtocol.Parameter("x", "i64")));
        var response = facts("editor-partial", List.of(function),
                List.of(new NagiAssistProtocol.Local("value", "i64", at(3, 5, 5), "local"),
                        new NagiAssistProtocol.Local("x", "i64", at(1, 11, 1), "parameter")),
                List.of(new NagiAssistProtocol.Reference(at(5, 5, 5), at(1, 5, 5)),
                        new NagiAssistProtocol.Reference(at(6, 5, 5), at(3, 5, 5)),
                        new NagiAssistProtocol.Reference(at(2, 12, 1), at(1, 11, 1))));
        var index = new NagiSemanticIndex(response);
        assertEquals(function, index.function(FILE, 5, 5, 5));
        assertNull("a same-name local must not inherit a function signature", index.function(FILE, 6, 5, 5));
        assertTrue(index.colors(FILE).stream().anyMatch(c -> c.location().equals(at(2, 12, 1))
                && c.attributes() == DefaultLanguageHighlighterColors.PARAMETER));
        assertTrue(index.colors(FILE).stream().anyMatch(c -> c.location().equals(at(5, 5, 5))
                && c.attributes() == DefaultLanguageHighlighterColors.FUNCTION_CALL));
    }
    @Test public void missingOrAmbiguousCompilerFactsNeverInventSignatures() {
        var a = new NagiAssistProtocol.Definition("same", "function", at(1, 5, 4), "def same(a: i64) -> i64", List.of());
        var b = new NagiAssistProtocol.Definition("same", "function", at(1, 5, 4), "def same(a: str) -> str", List.of());
        var references = List.of(new NagiAssistProtocol.Reference(at(4, 5, 4), at(1, 5, 4)));
        assertNull(new NagiSemanticIndex(facts("editor-partial", List.of(a, b), List.of(), references)).function(FILE, 4, 5, 4));
        var empty = new NagiSemanticIndex(new NagiAssistProtocol.Response("none", true, false, false, "names",
                List.of(), List.of(), references, List.of(FILE), List.of(a), List.of()));
        assertNull(empty.function(FILE, 4, 5, 4));
        assertTrue(empty.colors(FILE).isEmpty());
    }
    @Test public void acceptedFrontendSymbolsDoNotDependOnCompletionContext() {
        var function = new NagiAssistProtocol.Definition("add", "function", at(1, 5, 3), "def add(x: i64) -> i64",
                List.of(new NagiAssistProtocol.Parameter("x", "i64")));
        var index = new NagiSemanticIndex(facts("none", List.of(function), List.of(),
                List.of(new NagiAssistProtocol.Reference(at(4, 5, 3), function.location()))));
        assertEquals(function, index.function(FILE, 4, 5, 3));
        assertFalse(index.colors(FILE).isEmpty());
    }
    @Test public void explicitCompilerRolesOverrideGenericLocalPresentation() {
        var response = new NagiAssistProtocol.Response("editor-partial", true, true, false, "names", List.of(), List.of(),
                List.of(new NagiAssistProtocol.Reference(at(4, 8, 1), at(1, 9, 1))), List.of(FILE), List.of(),
                List.of(new NagiAssistProtocol.Local("x", "i64", at(1, 9, 1), "local")),
                List.of(new NagiAssistProtocol.SymbolRole("parameter", at(1, 9, 1)),
                        new NagiAssistProtocol.SymbolRole("field", at(5, 12, 1))));
        var index = new NagiSemanticIndex(response);
        assertTrue(index.colors(FILE).stream().anyMatch(c -> c.location().equals(at(4, 8, 1))
                && c.attributes() == DefaultLanguageHighlighterColors.PARAMETER));
        assertTrue(index.colors(FILE).stream().anyMatch(c -> c.location().equals(at(5, 12, 1))
                && c.attributes() == DefaultLanguageHighlighterColors.INSTANCE_FIELD));
    }

    @Test public void documentationIncludesDeclarationsClassesAndVirtualStandardTargets() {
        var point = new NagiAssistProtocol.Definition("Point", "class", at(1, 7, 5), "class Point\n    x: i64",
                List.of(), null, false, List.of(new NagiAssistProtocol.Parameter("x", "i64")));
        var standard = new NagiAssistProtocol.Definition("ready", "function",
                new NagiAssistProtocol.Location("stdlib:async", 1, 1, 5), "def ready() -> unit", List.of());
        var index = new NagiSemanticIndex(facts("none", List.of(point, standard), List.of(), List.of(
                new NagiAssistProtocol.Reference(at(4, 9, 5), point.location()),
                new NagiAssistProtocol.Reference(at(5, 9, 5), standard.location()))));
        assertEquals(point, index.definition(FILE, 1, 7, 5));
        assertEquals(point, index.definition(FILE, 4, 9, 5));
        assertEquals(standard, index.definition(FILE, 5, 9, 5));
        assertNull(index.definition(FILE, 4, 9, 4));
        assertNull("class fields are not function parameters", index.function(FILE, 4, 9, 5));
    }

    @Test public void conflictingOrUnresolvedReferencesFailClosedRegardlessOfOrder() {
        var a = new NagiAssistProtocol.Definition("same", "function", at(1, 5, 4), "def same() -> i64", List.of());
        var b = new NagiAssistProtocol.Definition("same", "function", at(2, 5, 4), "def same() -> str", List.of());
        var first = new NagiAssistProtocol.Reference(at(4, 5, 4), a.location());
        var second = new NagiAssistProtocol.Reference(at(4, 5, 4), b.location());
        var unresolved = new NagiAssistProtocol.Reference(at(4, 5, 4), at(8, 5, 4));
        for (var references : List.of(List.of(first, second), List.of(second, first),
                List.of(first, unresolved), List.of(unresolved, first))) {
            var index = new NagiSemanticIndex(facts("none", List.of(a, b), List.of(), references));
            assertNull(index.definition(FILE, 4, 5, 4));
            assertNull(index.function(FILE, 4, 5, 4));
            assertEquals(a, index.definition(FILE, 1, 5, 4));
        }
        var collision = new NagiSemanticIndex(facts("none", List.of(a, b), List.of(),
                List.of(new NagiAssistProtocol.Reference(a.location(), b.location()))));
        assertNull(collision.definition(FILE, 1, 5, 4));
    }    @Test public void localDocumentationRequiresExactUnambiguousCompilerFacts() {
        var local = new NagiAssistProtocol.Local("value", "i64", at(4, 5, 5), "local");
        var index = new NagiSemanticIndex(facts("none", List.of(), List.of(local), List.of()));
        assertEquals(local, index.local(FILE, 4, 5, 5));
        assertNull(index.local(FILE, 4, 6, 4));
        var other = new NagiAssistProtocol.Local("value", "str", local.location(), "local");
        assertNull(new NagiSemanticIndex(facts("none", List.of(), List.of(local, other), List.of())).local(FILE, 4, 5, 5));
        var references = List.of(new NagiAssistProtocol.Reference(local.location(), at(1, 1, 5)),
                new NagiAssistProtocol.Reference(local.location(), at(2, 1, 5)));
        assertNull(new NagiSemanticIndex(facts("none", List.of(), List.of(local), references)).local(FILE, 4, 5, 5));
        var a = new NagiAssistProtocol.Definition("value", "function", local.location(), "def value()", List.of());
        var b = new NagiAssistProtocol.Definition("value", "class", local.location(), "class value", List.of());
        assertNull(new NagiSemanticIndex(facts("none", List.of(a, b), List.of(local), List.of())).local(FILE, 4, 5, 5));
        var collision = new NagiSemanticIndex(facts("none", List.of(a), List.of(local), List.of()));
        assertNull(collision.local(FILE, 4, 5, 5));
        assertNull(collision.definition(FILE, 4, 5, 5));
        var unresolved = new NagiSemanticIndex(facts("none", List.of(a, b),
                List.of(new NagiAssistProtocol.Local("value", "i64", at(6, 5, 5), "local")),
                List.of(new NagiAssistProtocol.Reference(at(6, 5, 5), a.location()))));
        assertNull(unresolved.local(FILE, 6, 5, 5));
    }

}
