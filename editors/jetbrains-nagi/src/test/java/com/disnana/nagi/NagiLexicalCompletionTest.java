package com.disnana.nagi;

import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.Arrays;
import java.util.List;

/** Exercises the registered contributor with no compiler and an untrusted project. */
public final class NagiLexicalCompletionTest extends BasePlatformTestCase {
    private List<String> complete(String extension, String source) {
        myFixture.configureByText("hints" + extension, source);
        var items = myFixture.completeBasic();
        return items == null ? List.of() : Arrays.stream(items).map(item -> item.getLookupString()).toList();
    }
    public void testTryAndAllNonDeclarationKeywordsUseTheActualProviderInBothLanguages() {
        for (String extension : List.of(".nagi", ".low")) {
            for (String keyword : List.of("return", "if", "else", "while", "for", "match", "case", "async", "await",
                    "try", "scope", "spawn", "import", "from", "as", "extern", "in", "with", "and", "or", "not",
                    "True", "False", "None", "true", "false", "null")) {
                String prefix = keyword.substring(0, keyword.length() - 1);
                assertTrue(keyword + extension, complete(extension, prefix + "<caret>").contains(keyword));
            }
            assertTrue(complete(extension, extension.equals(".low")
                    ? "fn main() -> Result[str, Error] { let value = tr<caret>; }"
                    : "def main() -> Result[str, Error]:\n    value = tr<caret>\n").contains("try"));
        }
    }
    public void testTypeHintsAndPatternHintsRemainSeparateFromMembersAndImports() {
        for (String extension : List.of(".nagi", ".low")) {
            for (String type : NagiLexicalCompletion.TYPES) {
                var offered = complete(extension, "value: " + type.substring(0, 1) + "<caret>");
                assertTrue(type + extension, offered.contains(type));
                assertFalse(offered.contains("try"));
            }
            assertTrue(complete(extension, "case O<caret>").contains("Ok"));
            for (String source : List.of("value.tr<caret>", "value.\ntr<caret>", "value. # comment\ntr<caret>", "# tr<caret>", "\"tr<caret>",
                    "from module import tr<caret>", "import tr<caret>", "from \"x;y\" import tr<caret>", "def tr<caret>", "class tr<caret>")) {
                assertFalse(source, complete(extension, source).contains("try"));
            }
        }
    }
    public void testDeclarationLanguageAndTopLevelGuardsRemainIntact() {
        assertTrue(complete(".nagi", "cl<caret>").contains("class"));
        assertFalse(complete(".nagi", "rec<caret>").contains("record"));
        assertTrue(complete(".low", "rec<caret>").contains("record"));
        assertTrue(complete(".nagi", "async d<caret>").contains("def"));
        assertTrue(complete(".low", "extern f<caret>").contains("fn"));
        assertFalse(complete(".nagi", "def main():\n    cl<caret>").contains("class"));
        assertFalse(complete(".low", "fn main() { rec<caret> }").contains("record"));
        assertFalse(NagiLexicalCompletion.candidates("x".repeat(9000), 9000, false).contains("try"));
    }
}
