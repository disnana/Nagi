package com.disnana.nagi;

import java.util.HashMap;
import junit.framework.TestCase;

public final class NagiBracketDepthTest extends TestCase {
    public void testMatchingPairsShareTheirNestedDepth() {
        assertDepths("([()])", false, 0, 1, 2, 2, 1, 0);
        assertDepths("({[]})", true, 0, 1, 2, 2, 1, 0);
    }

    public void testStringsCommentsAndHighBracesKeepNormalSyntaxColor() {
        String high = "\"([x])\" # ([y])\n([]) {}\n";
        var depths = depths(high, false);
        int actual = high.lastIndexOf("([])");
        assertEquals(4, depths.size());
        assertEquals(Integer.valueOf(0), depths.get(actual));
        assertEquals(Integer.valueOf(1), depths.get(actual + 1));
        assertEquals(Integer.valueOf(1), depths.get(actual + 2));
        assertEquals(Integer.valueOf(0), depths.get(actual + 3));
    }

    public void testMalformedAndIncompletePairsAreNotInvented() {
        String text = "([)] (()";
        var depths = depths(text, false);
        int matched = text.lastIndexOf("()") + 1;
        assertEquals(2, depths.size());
        assertEquals(Integer.valueOf(1), depths.get(matched - 1));
        assertEquals(Integer.valueOf(1), depths.get(matched));
    }

    public void testDepthPaletteWrapsWithoutSeparatingAPair() {
        String text = "((((((()))))))";
        var depths = depths(text, false);
        assertEquals(Integer.valueOf(0), depths.get(0));
        assertEquals(Integer.valueOf(5), depths.get(5));
        assertEquals(Integer.valueOf(0), depths.get(6));
        assertEquals(Integer.valueOf(5), depths.get(8));
        assertEquals(Integer.valueOf(0), depths.get(text.length() - 1));
    }

    public void testLargeFilesRemainLinearAndAnnotationCountIsBounded() {
        String text = "()".repeat(10_000);
        long started = System.nanoTime();
        assertEquals(20_000, NagiBracketDepth.spans(text, false).size());
        long elapsedMillis = (System.nanoTime() - started) / 1_000_000;
        assertTrue("20k bracket scan took " + elapsedMillis + "ms", elapsedMillis < 3_000);
        assertTrue(NagiBracketDepth.spans("()".repeat(NagiBracketDepth.MAX_PAIRS + 1), false).isEmpty());
        assertTrue(NagiBracketDepth.spans("(".repeat(NagiBracketDepth.MAX_PAIRS + 1), false).isEmpty());
    }

    public void testColorSettingsExposeEveryDepth() {
        var page = new NagiColorSettingsPage();
        assertEquals(NagiBracketDepth.PALETTE_SIZE, page.getAttributeDescriptors().length);
        assertEquals(NagiBracketDepth.PALETTE_SIZE, page.getAdditionalHighlightingTagToDescriptorMap().size());
        for (int depth = 1; depth <= NagiBracketDepth.PALETTE_SIZE; depth++) {
            assertSame(NagiBracketColors.DEPTHS[depth - 1],
                    page.getAdditionalHighlightingTagToDescriptorMap().get("d" + depth));
        }
    }

    private static void assertDepths(String text, boolean low, int... expected) {
        var depths = depths(text, low);
        assertEquals(expected.length, depths.size());
        for (int offset = 0; offset < expected.length; offset++) {
            assertEquals("offset " + offset, Integer.valueOf(expected[offset]), depths.get(offset));
        }
    }
    private static HashMap<Integer, Integer> depths(String text, boolean low) {
        var result = new HashMap<Integer, Integer>();
        for (var span : NagiBracketDepth.spans(text, low)) {
            assertEquals(1, span.range().getLength());
            assertNull(result.put(span.range().getStartOffset(), span.paletteIndex()));
        }
        return result;
    }
}
