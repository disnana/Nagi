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

/** Strict adapter for responses from the compiler-owned editor assistance protocol. */
public final class NagiAssistProtocol {
    public static final int MAX_FILES = 128;
    public static final int MAX_OVERLAY_BYTES = 8_000_000;
    public static final int MAX_INPUT_BYTES = 16_000_000;
    public static final int MAX_OUTPUT_BYTES = 8_000_000;
    private static final int MAX_DIAGNOSTICS = 512;
    private static final int MAX_COMPLETIONS = 8192;
    private static final int MAX_REFERENCES = 20_000;
    private static final int MAX_SYMBOLS = 20_000;

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
    public record ImportContext(String kind, Location replacement, List<String> directories, boolean incomplete) {
        public ImportContext { directories = List.copyOf(directories); }
    }
    public record Reference(Location location, Location target) {}
    public record Parameter(String name, String type) {}
    public record Definition(String name, String kind, Location location, String signature,
                             List<Parameter> parameters, String returnType, boolean asynchronous,
                             List<Parameter> fields) {
        public Definition {
            parameters = List.copyOf(parameters);
            fields = List.copyOf(fields);
        }
        public Definition(String name, String kind, Location location, String signature, List<Parameter> parameters) {
            this(name, kind, location, signature, parameters, null, false, List.of());
        }
    }
    public record Local(String name, String type, Location location, String kind) {}
    public record SymbolRole(String kind, Location location) {}
    public record Response(String semanticStatus, boolean frontendChecked, boolean frontendAccepted,
                          boolean recovered, String completionKind, List<Diagnostic> diagnostics,
                          List<CompletionItem> completions, List<Reference> references, List<String> sourceFiles,
                          List<Definition> definitions, List<Local> locals, List<SymbolRole> roles,
                          ImportContext importContext, boolean hasSymbolIndex) {
        public Response(String semanticStatus, boolean frontendChecked, boolean frontendAccepted,
                        boolean recovered, String completionKind, List<Diagnostic> diagnostics,
                        List<CompletionItem> completions, List<Reference> references, List<String> sourceFiles,
                        List<Definition> definitions, List<Local> locals, List<SymbolRole> roles,
                        ImportContext importContext) {
            this(semanticStatus, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, references, sourceFiles, definitions, locals, roles, importContext, true);
        }
        public Response(String semanticStatus, boolean frontendChecked, boolean frontendAccepted,
                        boolean recovered, String completionKind, List<Diagnostic> diagnostics,
                        List<CompletionItem> completions, List<Reference> references, List<String> sourceFiles,
                        List<Definition> definitions, List<Local> locals, List<SymbolRole> roles) {
            this(semanticStatus, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, references, sourceFiles, definitions, locals, roles, null);
        }
        public Response(String semanticStatus, boolean frontendChecked, boolean frontendAccepted,
                        boolean recovered, String completionKind, List<Diagnostic> diagnostics,
                        List<CompletionItem> completions, List<Reference> references, List<String> sourceFiles) {
            this(semanticStatus, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, references, sourceFiles, List.of(), List.of(), List.of());
        }
        public Response(String semanticStatus, boolean frontendChecked, boolean frontendAccepted,
                        boolean recovered, String completionKind, List<Diagnostic> diagnostics,
                        List<CompletionItem> completions, List<Reference> references, List<String> sourceFiles,
                        List<Definition> definitions, List<Local> locals) {
            this(semanticStatus, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, references, sourceFiles, definitions, locals, List.of());
        }
        public Response {
            diagnostics = List.copyOf(diagnostics);
            completions = List.copyOf(completions);
            references = List.copyOf(references);
            sourceFiles = List.copyOf(sourceFiles);
            definitions = List.copyOf(definitions);
            locals = List.copyOf(locals);
            roles = List.copyOf(roles);
        }

        /** Only current source facts are replaced; query approval and diagnostics stay with the query response. */
        Response withSymbolIndex(Response baseline) {
            if (!baseline.hasSymbolIndex || frontendChecked != baseline.frontendChecked
                    || frontendAccepted != baseline.frontendAccepted || !diagnostics.equals(baseline.diagnostics))
                throw new IllegalArgumentException("queryless source facts disagree with the query verdict");
            return new Response(semanticStatus, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, baseline.references, baseline.sourceFiles,
                    baseline.definitions, baseline.locals, baseline.roles, importContext, true);
        }
    }

    /** Remove only the query from our own already-encoded immutable snapshot, preserving every overlay. */
    static byte[] withoutQuery(byte[] encoded) {
        JsonObject request = JsonParser.parseString(new String(encoded, StandardCharsets.UTF_8)).getAsJsonObject();
        request.remove("query");
        return request.toString().getBytes(StandardCharsets.UTF_8);
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

            ImportContext importContext = null;
            if (completion.has("context")) {
                String context = string(completion, "context");
                if (!Set.of("import-source", "import-names").contains(context)
                        || !completionKind.equals("names")) throw new IllegalArgumentException("invalid import completion context");
                Location replacement = requiredLocation(completion, "replacement");
                requireProtocolPath(replacement.file(), false);
                if ((long) replacement.column() + replacement.length() > Integer.MAX_VALUE)
                    throw new IllegalArgumentException("import replacement range exceeds its limit");
                List<String> directories = new ArrayList<>();
                if (completion.has("directories")) {
                    JsonArray paths = array(completion, "directories");
                    if (paths.size() > 1 || (context.equals("import-names") && !paths.isEmpty()))
                        throw new IllegalArgumentException("invalid import directories");
                    for (JsonElement element : paths) {
                        if (!element.isJsonPrimitive() || !element.getAsJsonPrimitive().isString())
                            throw new IllegalArgumentException("invalid import directory");
                        String directory = element.getAsString();
                        requireProtocolPath(directory, false);
                        Path actual = Path.of(physicalPath(directory));
                        Path parent = Path.of(physicalPath(replacement.file())).getParent();
                        if (parent == null || !actual.equals(parent))
                            throw new IllegalArgumentException("import directory is not the query parent");
                        directories.add(directory);
                    }
                }
                boolean incomplete = completion.has("incomplete") && bool(completion, "incomplete");
                if (incomplete && !completions.isEmpty()) throw new IllegalArgumentException("incomplete import enumeration has candidates");
                for (CompletionItem item : completions) {
                    if ((context.equals("import-source") && !item.kind().equals("module"))
                            || (context.equals("import-names") && !Set.of("function", "class", "enum", "resource").contains(item.kind())))
                        throw new IllegalArgumentException("invalid import candidate kind");
                }
                importContext = new ImportContext(context, replacement, directories, incomplete);
            } else if (completion.has("replacement") || completion.has("directories") || completion.has("incomplete")) {
                throw new IllegalArgumentException("import metadata requires a context");
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
            List<Definition> definitions = new ArrayList<>();
            List<Local> locals = new ArrayList<>();
            List<SymbolRole> roles = new ArrayList<>();
            JsonElement symbolsElement = root.get("symbols");
            boolean hasSymbolIndex = symbolsElement != null && !symbolsElement.isJsonNull();
            if (hasSymbolIndex) {
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
                // Older matching compiler responses can omit these additive facts.
                // Never derive a missing role, signature or type from IDE token text.
                for (JsonElement element : optionalArray(symbols, "definitions")) {
                    JsonObject item = object(element, "definition");
                    String kind = boundedString(item, "kind", 32);
                    if (!Set.of("function", "class", "resource", "enum").contains(kind)) {
                        throw new IllegalArgumentException("invalid definition kind");
                    }
                    List<Parameter> parameters = members(array(item, "parameters"));
                    String returnType = optionalString(item, "return_type");
                    if (returnType != null && (returnType.length() > 2048 || returnType.indexOf('\0') >= 0)) {
                        throw new IllegalArgumentException("return type exceeds bound");
                    }
                    definitions.add(new Definition(boundedString(item, "name", 256), kind,
                            requiredLocation(item, "location"), boundedString(item, "signature", 8192), parameters,
                            returnType, optionalBoolean(item, "asynchronous"), members(optionalArray(item, "fields"))));
                }
                for (JsonElement element : optionalArray(symbols, "locals")) {
                    JsonObject item = object(element, "local");
                    String kind = optionalString(item, "kind");
                    if (kind == null) kind = "local";
                    if (!Set.of("local", "parameter", "field").contains(kind)) throw new IllegalArgumentException("invalid local role");
                    locals.add(new Local(boundedString(item, "name", 256), boundedString(item, "type", 2048),
                            requiredLocation(item, "location"), kind));
                }
                for (JsonElement element : optionalArray(symbols, "roles")) {
                    JsonObject item = object(element, "symbol role");
                    String kind = string(item, "kind");
                    if (!Set.of("local", "parameter", "field").contains(kind)) throw new IllegalArgumentException("invalid symbol role");
                    roles.add(new SymbolRole(kind, requiredLocation(item, "location")));
                }
            }
            if (status.equals("none") && !completions.isEmpty()) throw new IllegalArgumentException("unverified candidates in empty semantic response");
            return new Response(status, frontendChecked, frontendAccepted, recovered, completionKind,
                    diagnostics, completions, references, sourceFiles, definitions, locals, roles, importContext, hasSymbolIndex);
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
    private static JsonArray optionalArray(JsonObject object, String name) {
        JsonElement value = object.get(name);
        if (value == null || value.isJsonNull()) return new JsonArray();
        JsonArray result = array(object, name);
        if (result.size() > MAX_SYMBOLS) throw new IllegalArgumentException("too many semantic symbols");
        return result;
    }
    private static List<Parameter> members(JsonArray values) {
        if (values.size() > 1024) throw new IllegalArgumentException("too many definition members");
        List<Parameter> members = new ArrayList<>();
        for (JsonElement value : values) {
            JsonObject member = object(value, "definition member");
            members.add(new Parameter(boundedString(member, "name", 256), boundedString(member, "type", 2048)));
        }
        return members;
    }

    private static String boundedString(JsonObject object, String name, int maximum) {
        String result = string(object, name);
        if (result.length() > maximum || result.indexOf('\0') >= 0) throw new IllegalArgumentException("invalid " + name);
        return result;
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
        if (!Path.of(physicalPath(value)).isAbsolute()) throw new IllegalArgumentException("source paths must be absolute");
    }

    /** Rust canonical Windows paths use verbatim drive/UNC prefixes. IDEA's
     * VFS uses the equivalent ordinary absolute spelling; preserve source identity. */
    static String physicalPath(String value) {
        String slashes = value.replace('\\', '/');
        if (slashes.regionMatches(true, 0, "//?/UNC/", 0, 8)) return "//" + slashes.substring(8);
        if (slashes.startsWith("//?/") && slashes.length() >= 7
                && Character.isLetter(slashes.charAt(4)) && slashes.charAt(5) == ':' && slashes.charAt(6) == '/')
            return slashes.substring(4);
        return value;
    }
}
