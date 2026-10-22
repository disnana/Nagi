package com.disnana.nagi;

import com.intellij.openapi.util.text.StringUtil;
import java.util.regex.Pattern;

/** Presentation only: semantic facts and diagnostic descriptions stay compiler-owned. */
final class NagiDocumentation {
    private static final Pattern HEADER = Pattern.compile("error: line ([1-9][0-9]*)(?::([1-9][0-9]*))?: (.*)");
    // Rust Debug strings containing escapes are deliberately left intact. Do
    // not guess how to decode a future compiler's token representation.
    private static final Pattern TOKEN = Pattern.compile("(.*) \\(found (Id|Num|Str|Sym)\\(\"([^\"\\\\]*)\"\\)\\)");
    private static final Pattern SIMPLE_TOKEN = Pattern.compile("(.*) \\(found (Newline|Indent|Dedent|Eof)\\)");

    private NagiDocumentation() {}

    static String escape(String text) { return StringUtil.escapeXmlEntities(text); }

    static String definition(NagiAssistProtocol.Definition definition) {
        StringBuilder html = new StringBuilder("<html><body><pre>")
                .append(escape(definition.signature())).append("</pre>");
        if (definition.kind().equals("function")) {
            if (!definition.parameters().isEmpty()) {
                html.append("<b>Parameters</b><ul>");
                for (var parameter : definition.parameters()) {
                    html.append("<li><code>").append(escape(parameter.name())).append(": ")
                            .append(escape(parameter.type())).append("</code></li>");
                }
                html.append("</ul>");
            }
            if (definition.returnType() != null) {
                html.append("<p><b>Returns:</b> <code>").append(escape(definition.returnType())).append("</code></p>");
            }
            if (definition.asynchronous()) html.append("<p>Async function</p>");
        } else if (definition.kind().equals("class") && !definition.fields().isEmpty()) {
            html.append("<b>Fields (use named arguments when constructing)</b><ul>");
            for (var field : definition.fields()) {
                html.append("<li><code>").append(escape(field.name())).append(": ")
                        .append(escape(field.type())).append("</code></li>");
            }
            html.append("</ul>");
        }
        return html.append("</body></html>").toString();
    }

    static String diagnosticTooltip(NagiAssistProtocol.Diagnostic diagnostic) {
        String[] lines = diagnostic.message().split("\n", -1);
        var header = HEADER.matcher(lines[0]);
        if (!header.matches() || diagnostic.line() == null
                || !header.group(1).equals(diagnostic.line().toString())) {
            return "<html><body><pre>" + escape(diagnostic.message()) + "</pre></body></html>";
        }
        String summary = readableToken(header.group(3));
        StringBuilder details = new StringBuilder();
        for (int index = 1; index < lines.length; index++) {
            String line = lines[index];
            String origin = " --> " + diagnostic.file() + ":" + diagnostic.line();
            if (diagnostic.file() != null && line.equals(origin)) {
                line = "Location: " + diagnostic.file() + ":" + diagnostic.line()
                        + (header.group(2) == null ? "" : ":" + header.group(2));
                String gutter = " " + diagnostic.line() + " | ";
                if (index + 1 < lines.length && lines[index + 1].startsWith(gutter)) {
                    details.append(line).append('\n').append("Source: ").append(lines[++index].substring(gutter.length()));
                    if (index + 1 < lines.length) details.append('\n');
                    continue;
                }
            }
            details.append(line);
            if (index + 1 < lines.length) details.append('\n');
        }
        return "<html><body><b>" + escape(summary) + "</b>"
                + (details.isEmpty() ? "" : "<pre>" + escape(details.toString()) + "</pre>") + "</body></html>";
    }

    private static String readableToken(String summary) {
        var token = TOKEN.matcher(summary);
        if (token.matches()) {
            String kind = switch (token.group(2)) {
                case "Id" -> "identifier";
                case "Num" -> "number";
                case "Str" -> "string";
                default -> "symbol";
            };
            return token.group(1) + " (found " + kind + ": " + token.group(3) + ")";
        }
        var simple = SIMPLE_TOKEN.matcher(summary);
        if (!simple.matches()) return summary;
        String kind = switch (simple.group(2)) {
            case "Newline" -> "newline";
            case "Indent" -> "indentation";
            case "Dedent" -> "end of indentation";
            default -> "end of file";
        };
        return simple.group(1) + " (found " + kind + ")";
    }
}
