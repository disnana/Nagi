package com.disnana.nagi;

import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.testFramework.EdtTestUtil;
import com.intellij.testFramework.fixtures.CompletionAutoPopupTestCase;
import com.intellij.util.ui.UIUtil;
import java.util.Arrays;
import java.util.concurrent.TimeUnit;

/** Official scoped test-mode opt-in; input still uses the normal TypedHandler route. */
public final class NagiAutomaticLookupTest extends CompletionAutoPopupTestCase {
    public void testTypedPrefixSchedulesNormalAutomaticLookup() throws Exception {
        boolean previous = EdtTestUtil.runInEdtAndGet(() -> {
            var settings = CodeInsightSettings.getInstance();
            boolean value = settings.AUTO_POPUP_COMPLETION_LOOKUP;
            settings.AUTO_POPUP_COMPLETION_LOOKUP = true;
            return value;
        });
        try {
            for (String extension : new String[] { ".nagi", ".low" }) {
                String expected = extension.equals(".nagi") ? "def" : "fn";
                EdtTestUtil.runInEdtAndWait(() -> {
                    myFixture.configureByText("automatic" + extension, "<caret>");
                    myFixture.type(extension.equals(".nagi") ? 'd' : 'f');
                });
                // Do not use typeWithPauses/joinCompletion: their SDK waits exceed this bound.
                long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(3);
                while (System.nanoTime() < deadline) {
                    boolean ready = EdtTestUtil.runInEdtAndGet(() ->
                            myFixture.getLookup() != null && myFixture.getLookup().getItems().stream()
                                    .anyMatch(item -> item.getLookupString().equals(expected)));
                    if (ready) break;
                    Thread.sleep(5);
                }
                EdtTestUtil.runInEdtAndWait(() -> {
                    assertNotNull("platform automatic lookup for " + expected, myFixture.getLookup());
                    assertTrue(myFixture.getLookup().getItems().stream()
                            .anyMatch(item -> item.getLookupString().equals(expected)));
                });
            }
        } finally {
            EdtTestUtil.runInEdtAndWait(() ->
                    CodeInsightSettings.getInstance().AUTO_POPUP_COMPLETION_LOOKUP = previous);
        }
    }

    public void testAutomaticLookupOffStillAllowsExplicitKeywordCompletion() throws Exception {
        EdtTestUtil.runInEdtAndWait(() -> {
            var settings = CodeInsightSettings.getInstance();
            boolean previous = settings.AUTO_POPUP_COMPLETION_LOOKUP;
            try {
                settings.AUTO_POPUP_COMPLETION_LOOKUP = false;
                for (String extension : new String[] { ".nagi", ".low" }) {
                    String expected = extension.equals(".nagi") ? "def" : "fn";
                    myFixture.configureByText("manual" + extension, "<caret>");
                    myFixture.type(extension.equals(".nagi") ? 'd' : 'f');
                    UIUtil.dispatchAllInvocationEvents();
                    assertNull("automatic lookup disabled for " + expected, myFixture.getLookup());
                    var offered = myFixture.completeBasic();
                    assertNotNull(offered);
                    assertTrue(Arrays.stream(offered)
                            .anyMatch(item -> item.getLookupString().equals(expected)));
                }
            } finally { settings.AUTO_POPUP_COMPLETION_LOOKUP = previous; }
        });
    }
}
