package com.disnana.nagi;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** Strict adapter for the compiler-owned, one-shot editor assistance protocol. */
public final class NagiAssistProtocol {
    public static final int MAX_FILES = 128;
    public static final int MAX_OVERLAY_BYTES = 8_000_000;
    public static final int MAX_INPUT_BYTES = 16_000_000;
    public static final int MAX_OUTPUT_BYTES = 8_000_000;
    private static final int MAX_DIAGNOSTICS = 512;
    private static final int MAX_COMPLETIONS = 8192;
    private static final int MAX_REFERENCES = 20_000;

    private NagiAssistProtocol() {}

    public record Overlay(String file, String text) {
        public Overlay {
            if (file == null || file.isBlank() || text == null) throw new IllegalArgumentException("invalid overlay");
            requireProtocolPath(file, false);
        }
    }

    /** Compiler positions are one-based and columns count UTF-16 code units. */
    public record Query(String file, int line, int column) {
        public Query {
            if (file == null || file.isBlank() || line < 1 || column < 1) throw new IllegalArgumentException("invalid query");
            requireProtocolPath(file, false);
        }
    }

    public record Request(List<Overlay> files, Query query) {
        public Request { files = List.copyOf(files); }
    }

    public record Location(String file, int line, int column, int length) {
        public Location {
            requireProtocolPath(file, true);
            if (line < 1 || column < 1 || length < 0) throw new IllegalArgumentException("invalid location");
        }
    }
    public record Diagnostic(String message, String file, Integer line, Location range) {}
    public record CompletionItem(String name, String kind, String type, String signature,
                                Location target, boolean borrowed, String access) {}
    public record Reference(Location location, Location target) {}
    public record Response(String semanticStatus, boolean frontendChecked, boolean frontendAccepted,
                          boolean recovered, String completionKind, List<Diagnostic> diagnostics,
                          List<CompletionItem> completions, List<Reference> references, List<String> sourceFiles) {
        public Response {
            diagnostics = List.copyOf(diagnostics);
            completions = List.copyOf(completions);
            references = List.copyOf(references);
            sourceFiles = List.copyOf(sourceFiles);
        }
    }

    public static byte[] encode(Request request) {
        if (request.files().size() > MAX_FILES) throw new IllegalArgumentException("too many editor overlays");
        JsonObject root = new JsonObject();
        JsonArray files = new JsonArray();
        Set<String> paths = new HashSet<>();
        long overlayBytes = 0;
        for (Overlay overlay : request.files()) {
            if (!paths.add(overlay.file())) throw new IllegalArgumentException("duplicate editor overlay path");
            byte[] text = overlay.text().getBytes(StandardCharsets.UTF_8);
            overlayBytes += text.length;
            if (overlayBytes > MAX_OVERLAY_BYTES) throw new IllegalArgumentException("editor overlays exceed 8 MB");
            JsonObject file = new JsonObject();
            file.addProperty("file", overlay.file());
            file.addProperty("text", overlay.text());
            files.add(file);
        }
        root.add("files", files);
        if (request.query() != null) {
            JsonObject query = new JsonObject();
            query.addProperty("file", request.query().file());
            query.addProperty("line", request.query().line());
            query.addProperty("column", request.query().column());
            root.add("query", query);
        }
        byte[] encoded = root.toString().getBytes(StandardCharsets.UTF_8);
        if (encoded.length > MAX_INPUT_BYTES) throw new IllegalArgumentException("editor input exceeds 16 MB");
        return encoded;
    }

    public static Response decode(byte[] output) {
        if (output.length > MAX_OUTPUT_BYTES) throw new IllegalArgumentException("editor response exceeds 8 MB");
        try {
            String json = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(output)).toString();
            JsonElement parsed = JsonParser.parseString(json);
            JsonObject root = object(parsed, "response");
            if (!"nagi-assist-v1".equals(string(root, "format"))) throw new IllegalArgumentException("unsupported editor response format");
            String status = string(root, "semantic_status");
            if (!status.equals("none") && !status.equals("editor-partial")) throw new IllegalArgumentException("invalid semantic status");
            boolean frontendChecked = bool(root, "frontend_checked");
            boolean frontendAccepted = bool(root, "frontend_accepted");
            if (frontendAccepted && !frontendChecked) throw new IllegalArgumentException("frontend acceptance requires frontend checking");
            if (bool(root, "full_compile_checked")) throw new IllegalArgumentException("editor response claims a full compile");
            boolean recovered = bool(root, "recovered");

            JsonObject completion = object(required(root, "completion"), "completion");
            String completionKind = string(completion, "kind");
            if (!completionKind.equals("names") && !completionKind.equals("members")) throw new IllegalArgumentException("invalid completion kind");
            if (!"read".equals(string(completion, "access"))) throw new IllegalArgumentException("invalid completion access");
            JsonArray completionArray = array(completion, "items");
            if (completionArray.size() > MAX_COMPLETIONS) throw new IllegalArgumentException("too many completion items");
            List<CompletionItem> completions = new ArrayList<>(completionArray.size());
            for (JsonElement element : completionArray) {
                JsonObject item = object(element, "completion item");
                String name = string(item, "name");
                if (name.isBlank() || name.length() > 256 || name.indexOf('\n') >= 0 || name.indexOf('\r') >= 0) throw new IllegalArgumentException("invalid completion name");
                String kind = string(item, "kind");
                if (!Set.of("local", "field", "function", "class", "resource", "enum", "module").contains(kind)) {
                    throw new IllegalArgumentException("invalid completion item kind");
                }
                String access = string(item, "access");
                if (!access.equals("read") && !access.equals("namespace")) throw new IllegalArgumentException("invalid completion item access");
                String type = optionalString(item, "type");
                String signature = optionalString(item, "signature");
                if ((type != null && type.length() > 512) || (signature != null && signature.length() > 512)) {
                    throw new IllegalArgumentException("completion detail exceeds its limit");
                }
                completions.add(new CompletionItem(name, kind, type, signature,
                        optionalLocation(item.get("target")), optionalBoolean(item, "borrowed"), access));
            }

            JsonArray diagnosticArray = array(root, "diagnostics");
            if (diagnosticArray.size() > MAX_DIAGNOSTICS) throw new IllegalArgumentException("too many diagnostics");
            List<Diagnostic> diagnostics = new ArrayList<>(diagnosticArray.size());
            for (JsonElement element : diagnosticArray) {
                JsonObject item = object(element, "diagnostic");
                if (!"error".equals(string(item, "severity"))) throw new IllegalArgumentException("unsupported diagnostic severity");
                String stage = string(item, "stage");
                if (!stage.equals("load") && !stage.equals("check")) throw new IllegalArgumentException("unsupported diagnostic stage");
                String message = string(item, "message");
                String file = optionalString(item, "file");
                if (file != null) requireProtocolPath(file, true);
                Integer line = optionalPositiveInt(item, "line");
                Location range = null;
                if (item.has("range") && !item.get("range").isJsonNull()) {
                    JsonObject rangeObject = object(item.get("range"), "diagnostic range");
                    if (file == null || file.isBlank()) throw new IllegalArgumentException("diagnostic range has no source file");
                    range = new Location(file, positiveInt(rangeObject, "line"),
                            positiveInt(rangeObject, "column"), nonNegativeInt(rangeObject, "length"));
                    if (line != null && !line.equals(range.line())) throw new IllegalArgumentException("diagnostic range line mismatch");
                }
                diagnostics.add(new Diagnostic(message, file, line, range));
            }

            List<Reference> references = new ArrayList<>();
            List<String> sourceFiles = new ArrayList<>();
            JsonElement symbolsElement = root.get("symbols");
            if (symbolsElement != null && !symbolsElement.isJsonNull()) {
                JsonObject symbols = object(symbolsElement, "symbols");
                if (!"nagi-symbols-v1".equals(string(symbols, "format"))) throw new IllegalArgumentException("unsupported symbols format");
                JsonArray sourceFileArray = array(symbols, "files");
                if (sourceFileArray.size() > MAX_FILES) throw new IllegalArgumentException("too many source files");
                for (JsonElement sourceFile : sourceFileArray) {
                    if (!sourceFile.isJsonPrimitive() || !sourceFile.getAsJsonPrimitive().isString()) {
                        throw new IllegalArgumentException("invalid symbols file path");
                    }
                    String path = sourceFile.getAsString();
                    requireProtocolPath(path, true);
                    sourceFiles.add(path);
                }
                JsonArray referenceArray = array(symbols, "references");
                if (referenceArray.size() > MAX_REFERENCES) throw new IllegalArgumentException("too many navigation references");
                for (JsonElement element : referenceArray) {
                    JsonObject reference = object(element, "symbol reference");
                    Location location = requiredLocation(reference, "location");
                    Location target = optionalLocation(reference.get("target"));
                    if (target != null) references.add(new Reference(location, target));
                }
            }
            if (status.equals("none") && !completions.isEmpty()) throw new IllegalArgumentException("unverified candidates in empty semantic response");
            return new Response(status, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, references, sourceFiles);
        } catch (IllegalStateException | NumberFormatException | ArithmeticException | CharacterCodingException | JsonParseException exception) {
            throw new IllegalArgumentException("malformed editor response", exception);
        }
    }

    private static JsonElement required(JsonObject object, String name) {
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull()) throw new IllegalArgumentException("missing " + name);
        return value;
    }
    private static JsonObject object(JsonElement value, String label) {
        if (value == null || !value.isJsonObject()) throw new IllegalArgumentException("invalid " + label);
        return value.getAsJsonObject();
    }
    private static JsonArray array(JsonObject object, String name) {
        JsonElement value = required(object, name);
        if (!value.isJsonArray()) throw new IllegalArgumentException("invalid " + name);
        return value.getAsJsonArray();
    }
    private static String string(JsonObject object, String name) {
        JsonElement value = required(object, name);
        if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) throw new IllegalArgumentException("invalid " + name);
        return value.getAsString();
    }
    private static String optionalString(JsonObject object, String name) {
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull()) return null;
        if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) throw new IllegalArgumentException("invalid " + name);
        return value.getAsString();
    }
    private static boolean bool(JsonObject object, String name) {
        JsonElement value = required(object, name);
        if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isBoolean()) throw new IllegalArgumentException("invalid " + name);
        return value.getAsBoolean();
    }
    private static boolean optionalBoolean(JsonObject object, String name) {
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull()) return false;
        if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isBoolean()) throw new IllegalArgumentException("invalid " + name);
        return value.getAsBoolean();
    }
    private static int positiveInt(JsonObject object, String name) {
        int value = integer(object, name);
        if (value < 1) throw new IllegalArgumentException("invalid " + name);
        return value;
    }
    private static int nonNegativeInt(JsonObject object, String name) {
        int value = integer(object, name);
        if (value < 0) throw new IllegalArgumentException("invalid " + name);
        return value;
    }
    private static Integer optionalPositiveInt(JsonObject object, String name) {
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull()) return null;
        return positiveInt(object, name);
    }
    private static int integer(JsonObject object, String name) {
        JsonElement value = required(object, name);
        if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()) throw new IllegalArgumentException("invalid " + name);
        long raw;
        try { raw = value.getAsBigDecimal().longValueExact(); }
        catch (ArithmeticException exception) { throw new IllegalArgumentException("invalid " + name, exception); }
        if (raw < Integer.MIN_VALUE || raw > Integer.MAX_VALUE) throw new IllegalArgumentException("invalid " + name);
        return (int) raw;
    }
    private static Location requiredLocation(JsonObject object, String name) {
        JsonElement value = required(object, name);
        return location(value);
    }
    private static Location optionalLocation(JsonElement value) {
        if (value == null || value.isJsonNull()) return null;
        return location(value);
    }
    private static Location location(JsonElement value) {
        JsonObject object = object(value, "location");
        return new Location(string(object, "file"), positiveInt(object, "line"), positiveInt(object, "column"), nonNegativeInt(object, "length"));
    }

    private static void requireProtocolPath(String value, boolean allowStandardSource) {
        if (value == null || value.isBlank() || value.indexOf('\0') >= 0) throw new IllegalArgumentException("invalid source path");
        if (allowStandardSource && value.startsWith("stdlib:")) return;
        if (!Path.of(value).isAbsolute()) throw new IllegalArgumentException("source paths must be absolute");
    }
}
