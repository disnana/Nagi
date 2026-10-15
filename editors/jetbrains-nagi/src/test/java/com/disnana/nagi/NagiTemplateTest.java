package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.codeInsight.template.TemplateActionContext;
import com.intellij.codeInsight.template.TemplateManager;
import com.intellij.codeInsight.template.impl.TemplateManagerImpl;
import com.intellij.codeInsight.template.impl.TemplateSettings;
import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.psi.codeStyle.CodeStyleSettingsManager;

/** Exercises installed templates and real placeholder navigation, with no compiler process. */
public final class NagiTemplateTest extends BasePlatformTestCase {
    @Override protected void setUp() throws Exception {
        super.setUp();
        TemplateManagerImpl.setTemplateTesting(getTestRootDisposable());
    }

    public void testTemplateContextsDoNotOfferPythonSyntaxOrExpandWhileTyping() {
        var high = new NagiTemplateContext();
        var low = new NagiTemplateContext.Low();
        for (String content : new String[]{"", "def", "adef"}) {
            var file = myFixture.configureByText("sample.nagi", content);
            assertTrue(high.isInContext(TemplateActionContext.expanding(file, 0)));
            assertFalse(low.isInContext(TemplateActionContext.expanding(file, 0)));
        }
        for (String content : new String[]{"# def", "\"def\"", "    def", "value = def"}) {
            var file = myFixture.configureByText("sample.nagi", content);
            assertFalse(content, high.isInContext(TemplateActionContext.expanding(file, content.indexOf("def"))));
        }
        var file = myFixture.configureByText("sample.low", "fn");
        assertTrue(low.isInContext(TemplateActionContext.expanding(file, 0)));
        assertFalse(high.isInContext(TemplateActionContext.expanding(file, 0)));
        myFixture.configureByText("sample.nagi", "<caret>");
        myFixture.type("def");
        myFixture.checkResult("def<caret>");
        assertNull(TemplateManager.getInstance(getProject()).getActiveTemplate(myFixture.getEditor()));
        for (String invalid : new String[]{"pass", "break", "continue", "elif"}) {
            assertNull(TemplateSettings.getInstance().getTemplate(invalid, "Nagi"));
        }
    }

    public void testHighLowAndAsyncTemplatesRespectIndentWidthAndTabOrder() {
        for (String key : new String[]{"def", "adef", "fn"}) {
            for (int width : new int[]{2, 4, 8}) {
                var previous = CodeStyleSettingsManager.getInstance(getProject()).getTemporarySettings();
                var settings = CodeStyle.getSettings(getProject()).clone();
                var language = key.equals("fn") ? NagiLanguage.LOW : NagiLanguage.HIGH;
                var options = settings.getLanguageIndentOptions(language);
                options.INDENT_SIZE = width;
                options.USE_TAB_CHARACTER = true;
                CodeStyle.setTemporarySettings(getProject(), settings);
                try {
                    // A new virtual file prevents the fixture's previous per-file style cache
                    // from surviving a deliberately replaced temporary project scheme.
                    var file = myFixture.configureByText(key + width + (key.equals("fn") ? ".low" : ".nagi"), "<caret>");
                    assertEquals("effective width before template " + key, width, CodeStyle.getIndentOptions(file).INDENT_SIZE);
                    var template = TemplateSettings.getInstance().getTemplate(key, "Nagi");
                    assertNotNull("registered template " + key, template);
                    WriteCommandAction.runWriteCommandAction(getProject(), () ->
                            TemplateManager.getInstance(getProject()).startTemplate(myFixture.getEditor(), template));
                    var state = TemplateManagerImpl.getTemplateState(myFixture.getEditor());
                    assertNotNull(state);
                    for (String selection : new String[]{"compute", "value", "i64", "i64", key.equals("fn") ? "return 0;" : "return 0"}) {
                        assertEquals(key, selection, myFixture.getEditor().getSelectionModel().getSelectedText());
                        WriteCommandAction.runWriteCommandAction(getProject(), state::nextTab);
                    }
                    String expected = key.equals("fn")
                            ? "fn compute(value: i64) -> i64 {\n" + " ".repeat(width) + "return 0;\n}\n<caret>"
                            : (key.equals("adef") ? "async " : "") + "def compute(value: i64) -> i64:\n"
                                    + " ".repeat(width) + "return 0\n<caret>";
                    myFixture.checkResult(expected);
                    assertFalse(myFixture.getEditor().getDocument().getText().contains("\t"));
                } finally {
                    TemplateManager.getInstance(getProject()).finishTemplate(myFixture.getEditor());
                    if (previous == null) CodeStyle.dropTemporarySettings(getProject());
                    else CodeStyle.setTemporarySettings(getProject(), previous);
                }
            }
        }
    }
}
