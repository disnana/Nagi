package com.disnana.nagi;

import com.intellij.lang.parameterInfo.ParameterInfoUIContext;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

public final class NagiParameterInfoTest extends BasePlatformTestCase {
    private void call(String name, String before, String callee, int parameter) {
        myFixture.configureByText(name, before);
        var call = NagiParameterInfoHandler.callAt(myFixture.getFile(), myFixture.getEditor(), myFixture.getCaretOffset());
        assertNotNull(call);
        assertEquals(callee, call.callee().getText());
        assertEquals(parameter, call.parameter());
        assertEquals("(", call.opening().getText());
    }
    public void testHighLowNestedListsStringsAndIncompleteCalls() {
        call("args.nagi", "def main():\n    add(1, <caret>)", "add", 1);
        call("nested.low", "fn main() { add(other(1, 2), [3, 4], <caret>); }", "add", 2);
        call("list.nagi", "def main():\n    add(1, [2, 3<caret>", "add", 1);
        call("string.nagi", "def main():\n    add(\"x,y\", <caret>", "add", 1);
        call("inner.nagi", "def main():\n    add(1, other(2, <caret>", "other", 1);
        call("unicode.nagi", "# 日本語\r\ndef main():\r\n    add(\"😀\", <caret>", "add", 1);
    }
    public void testDefinitionsBrokenDelimitersAndBoundedLookbackHaveNoCall() {
        for (String text : List.of("def add(x: i64, <caret>", "fn add(x: i64, <caret>", "def main():\n    add([1), <caret>",
                "def main():\n    add(" + " " .repeat(9000) + "<caret>")) {
            myFixture.configureByText(text.startsWith("fn") ? "unknown.low" : "unknown.nagi", text);
            assertNull(NagiParameterInfoHandler.callAt(myFixture.getFile(), myFixture.getEditor(), myFixture.getCaretOffset()));
        }
    }
    public void testSignaturePresentationHighlightsOnlyTheCurrentCompilerParameter() {
        var shown = new AtomicReference<Object[]>();
        ParameterInfoUIContext ui = (ParameterInfoUIContext) Proxy.newProxyInstance(getClass().getClassLoader(),
                new Class<?>[] {ParameterInfoUIContext.class}, (proxy, method, args) -> switch (method.getName()) {
                    case "getCurrentParameterIndex" -> 1;
                    case "getDefaultParameterColor" -> java.awt.Color.WHITE;
                    case "setupUIComponentPresentation" -> { shown.set(args); yield args[0]; }
                    default -> method.getReturnType() == boolean.class ? false : null;
                });
        var definition = new NagiAssistProtocol.Definition("add", "function", new NagiAssistProtocol.Location(
                Path.of(System.getProperty("java.io.tmpdir"), "args.nagi").toAbsolutePath().toString(), 1, 5, 3),
                "def add(a: i64, b: str) -> i64", List.of(new NagiAssistProtocol.Parameter("a", "i64"),
                new NagiAssistProtocol.Parameter("b", "str")));
        new NagiParameterInfoHandler().updateUI(definition, ui);
        assertEquals("add(a: i64, b: str)", shown.get()[0]);
        String text = (String) shown.get()[0];
        assertEquals("b: str", text.substring((Integer) shown.get()[1], (Integer) shown.get()[2]));
    }
}
