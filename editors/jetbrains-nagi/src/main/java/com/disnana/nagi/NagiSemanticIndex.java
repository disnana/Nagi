package com.disnana.nagi;

import com.intellij.openapi.editor.DefaultLanguageHighlighterColors;
import com.intellij.openapi.editor.colors.TextAttributesKey;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Immutable location index, constructed on the response worker, never by name matching. */
final class NagiSemanticIndex {
    record Key(String file, int line, int column, int length) {
        static Key of(NagiAssistProtocol.Location location) {
            return new Key(NagiAssistService.normalizePath(location.file()), location.line(), location.column(), location.length());
        }
    }
    record Color(NagiAssistProtocol.Location location, TextAttributesKey attributes) {}
    private final Map<Key, NagiAssistProtocol.Definition> functions;
    private final Map<String, List<Color>> colors;

    NagiSemanticIndex(NagiAssistProtocol.Response response) {
        // semantic_status describes cursor completion, not the whole symbol
        // graph. A successful frontend still supplies checked facts when no
        // completion context exists (for example, between call arguments).
        if (!response.frontendAccepted() && !response.semanticStatus().equals("editor-partial")) {
            functions = Map.of();
            colors = Map.of();
            return;
        }
        Map<Key, NagiAssistProtocol.Definition> definitions = new HashMap<>();
        Set<Key> ambiguous = new HashSet<>();
        Map<Key, Color> classified = new HashMap<>();
        for (NagiAssistProtocol.Definition definition : response.definitions()) {
            Key key = Key.of(definition.location());
            var previous = definitions.putIfAbsent(key, definition);
            if (previous != null && !previous.equals(definition)) ambiguous.add(key);
            TextAttributesKey attributes = switch (definition.kind()) {
                case "function" -> DefaultLanguageHighlighterColors.FUNCTION_DECLARATION;
                case "class", "resource", "enum" -> DefaultLanguageHighlighterColors.CLASS_NAME;
                default -> null;
            };
            if (attributes != null) classified.put(key, new Color(definition.location(), attributes));
        }
        for (var local : response.locals()) {
            TextAttributesKey attributes = switch (local.kind()) {
                case "parameter" -> DefaultLanguageHighlighterColors.PARAMETER;
                case "field" -> DefaultLanguageHighlighterColors.INSTANCE_FIELD;
                default -> DefaultLanguageHighlighterColors.LOCAL_VARIABLE;
            };
            classified.put(Key.of(local.location()), new Color(local.location(), attributes));
        }
        for (var role : response.roles()) {
            TextAttributesKey attributes = switch (role.kind()) {
                case "parameter" -> DefaultLanguageHighlighterColors.PARAMETER;
                case "field" -> DefaultLanguageHighlighterColors.INSTANCE_FIELD;
                default -> DefaultLanguageHighlighterColors.LOCAL_VARIABLE;
            };
            classified.put(Key.of(role.location()), new Color(role.location(), attributes));
        }
        for (Key key : ambiguous) { definitions.remove(key); classified.remove(key); }
        Map<Key, NagiAssistProtocol.Definition> callable = new HashMap<>();
        for (var reference : response.references()) {
            Key target = Key.of(reference.target());
            var definition = definitions.get(target);
            if (definition != null && definition.kind().equals("function")) callable.put(Key.of(reference.location()), definition);
            Color color = classified.get(target);
            if (color != null) {
                TextAttributesKey attributes = color.attributes();
                if (definition != null && definition.kind().equals("function")
                        && !Key.of(reference.location()).equals(target)) attributes = DefaultLanguageHighlighterColors.FUNCTION_CALL;
                classified.put(Key.of(reference.location()), new Color(reference.location(), attributes));
            }
        }
        Map<String, List<Color>> byFile = new HashMap<>();
        for (Color color : classified.values()) {
            if (color.location().length() > 0 && !color.location().file().startsWith("stdlib:")) {
                byFile.computeIfAbsent(Key.of(color.location()).file(), ignored -> new ArrayList<>()).add(color);
            }
        }
        Map<String, List<Color>> frozen = new HashMap<>();
        byFile.forEach((path, values) -> frozen.put(path, List.copyOf(values)));
        colors = Map.copyOf(frozen);
        functions = Map.copyOf(callable);
    }

    NagiAssistProtocol.Definition function(String file, int line, int column, int length) {
        return functions.get(new Key(NagiAssistService.normalizePath(file), line, column, length));
    }
    List<Color> colors(String file) { return colors.getOrDefault(NagiAssistService.normalizePath(file), List.of()); }
}
