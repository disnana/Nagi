package com.disnana.nagi;

import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.testFramework.EdtTestUtil;
import com.intellij.testFramework.fixtures.CompletionAutoPopupTestCase;
import com.intellij.util.ui.UIUtil;
import java.util.Arrays;
import java.util.concurrent.TimeUnit;

/** Official scoped test-mode opt-in; input still uses the normal TypedHandler route. */
public final class NagiAutomaticLookupTest extends CompletionAutoPopupTestCase {
    public void testTypedPrefixSchedulesNormalAutomaticLookupForHigh() throws Exception {
        assertAutomaticLookup(".nagi", 'd', "def");
    }

    public void testTypedPrefixSchedulesNormalAutomaticLookupForLow() throws Exception {
        assertAutomaticLookup(".low", 'f', "fn");
    }

    private void assertAutomaticLookup(String extension, char typed, String expected) throws Exception {
        boolean previous = EdtTestUtil.runInEdtAndGet(() -> {
            var settings = CodeInsightSettings.getInstance();
            boolean value = settings.AUTO_POPUP_COMPLETION_LOOKUP;
            settings.AUTO_POPUP_COMPLETION_LOOKUP = true;
            return value;
        });
        try {
            EdtTestUtil.runInEdtAndWait(() -> {
                myFixture.configureByText("automatic" + extension, "<caret>");
                myFixture.type(typed);
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
        } finally {
            EdtTestUtil.runInEdtAndWait(() ->
                    CodeInsightSettings.getInstance().AUTO_POPUP_COMPLETION_LOOKUP = previous);
        }
    }

    public void testAutomaticLookupOffStillAllowsExplicitKeywordCompletionForHigh() throws Exception {
        assertExplicitKeywordCompletionWithAutomaticLookupOff(".nagi", 'd', "def");
    }

    public void testAutomaticLookupOffStillAllowsExplicitKeywordCompletionForLow() throws Exception {
        assertExplicitKeywordCompletionWithAutomaticLookupOff(".low", 'f', "fn");
    }

    private void assertExplicitKeywordCompletionWithAutomaticLookupOff(
            String extension, char typed, String expected) throws Exception {
        EdtTestUtil.runInEdtAndWait(() -> {
            var settings = CodeInsightSettings.getInstance();
            boolean previous = settings.AUTO_POPUP_COMPLETION_LOOKUP;
            try {
                settings.AUTO_POPUP_COMPLETION_LOOKUP = false;
                myFixture.configureByText("manual" + extension, "<caret>");
                myFixture.type(typed);
                UIUtil.dispatchAllInvocationEvents();
                assertNull("automatic lookup disabled for " + expected, myFixture.getLookup());
                var offered = myFixture.completeBasic();
                assertNotNull(offered);
                assertTrue(Arrays.stream(offered)
                        .anyMatch(item -> item.getLookupString().equals(expected)));
            } finally {
                settings.AUTO_POPUP_COMPLETION_LOOKUP = previous;
            }
        });
    }
}
