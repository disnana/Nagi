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
    private final Map<Key, NagiAssistProtocol.Definition> documented;
    private final Map<String, List<Color>> colors;
    private final Map<Key, NagiAssistProtocol.Local> localDescriptions;

    NagiSemanticIndex(NagiAssistProtocol.Response response) {
        // semantic_status describes cursor completion, not the whole symbol
        // graph. A successful frontend still supplies checked facts when no
        // completion context exists (for example, between call arguments).
        if (!response.frontendAccepted() && !response.semanticStatus().equals("editor-partial")) {
            functions = Map.of();
            documented = Map.of();
            colors = Map.of();
            localDescriptions = Map.of();
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
        Map<Key, Key> targets = new HashMap<>();
        Set<Key> conflictingReferences = new HashSet<>();
        for (var reference : response.references()) {
            Key location = Key.of(reference.location());
            Key target = Key.of(reference.target());
            Key previous = targets.putIfAbsent(location, target);
            if (previous != null && !previous.equals(target)) conflictingReferences.add(location);
        }
        Set<Key> ambiguousTargets = new HashSet<>(conflictingReferences);
        Map<Key, NagiAssistProtocol.Definition> descriptions = new HashMap<>(definitions);
        Map<Key, NagiAssistProtocol.Definition> callable = new HashMap<>();
        for (var reference : response.references()) {
            Key location = Key.of(reference.location());
            Key target = Key.of(reference.target());
            var definition = definitions.get(target);
            if (definition == null || conflictingReferences.contains(location)) {
                // An unresolved/conflicting reference must not inherit a
                // coincident declaration's presentation either.
                conflictingReferences.add(location);
            } else {
                var previous = descriptions.putIfAbsent(location, definition);
                if (previous != null && !previous.equals(definition)) conflictingReferences.add(location);
                if (definition.kind().equals("function")) callable.put(location, definition);
            }
            Color color = classified.get(target);
            if (color != null) {
                TextAttributesKey attributes = color.attributes();
                if (definition != null && definition.kind().equals("function")
                        && !Key.of(reference.location()).equals(target)) attributes = DefaultLanguageHighlighterColors.FUNCTION_CALL;
                classified.put(Key.of(reference.location()), new Color(reference.location(), attributes));
            }
        }
        for (Key key : conflictingReferences) { descriptions.remove(key); callable.remove(key); }
        for (Key key : ambiguous) { descriptions.remove(key); callable.remove(key); }
        Map<String, List<Color>> byFile = new HashMap<>();
        for (Color color : classified.values()) {
            if (color.location().length() > 0 && !color.location().file().startsWith("stdlib:")) {
                byFile.computeIfAbsent(Key.of(color.location()).file(), ignored -> new ArrayList<>()).add(color);
            }
        }
        Map<String, List<Color>> frozen = new HashMap<>();
        byFile.forEach((path, values) -> frozen.put(path, List.copyOf(values)));
        Map<Key, NagiAssistProtocol.Local> locals = new HashMap<>();
        Set<Key> conflictingLocals = new HashSet<>();
        for (var local : response.locals()) {
            Key key = Key.of(local.location());
            var previous = locals.putIfAbsent(key, local);
            if (previous != null && !previous.equals(local)) conflictingLocals.add(key);
        }
        Set<Key> collidingLocalKeys = new HashSet<>(locals.keySet());
        collidingLocalKeys.retainAll(descriptions.keySet());
        for (var reference : response.references()) {
            if (ambiguous.contains(Key.of(reference.target()))) conflictingLocals.add(Key.of(reference.location()));
        }
        conflictingLocals.addAll(collidingLocalKeys);
        collidingLocalKeys.forEach(descriptions::remove);
        collidingLocalKeys.forEach(callable::remove);
        // A local's own exact type is authoritative even without a navigable
        // definition. It cannot resurrect conflicting targets or declarations.
        conflictingLocals.addAll(ambiguousTargets);
        conflictingLocals.addAll(ambiguous);
        conflictingLocals.addAll(definitions.keySet());
        conflictingLocals.forEach(locals::remove);
        localDescriptions = Map.copyOf(locals);
        colors = Map.copyOf(frozen);
        functions = Map.copyOf(callable);
        documented = Map.copyOf(descriptions);
    }

    NagiAssistProtocol.Definition function(String file, int line, int column, int length) {
        return functions.get(new Key(NagiAssistService.normalizePath(file), line, column, length));
    }
    NagiAssistProtocol.Definition definition(String file, int line, int column, int length) {
        return documented.get(new Key(NagiAssistService.normalizePath(file), line, column, length));
    }
    NagiAssistProtocol.Local local(String file, int line, int column, int length) {
        return localDescriptions.get(new Key(NagiAssistService.normalizePath(file), line, column, length));
    }
    List<Color> colors(String file) { return colors.getOrDefault(NagiAssistService.normalizePath(file), List.of()); }
}
