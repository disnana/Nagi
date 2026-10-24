package com.disnana.nagi;

import com.intellij.openapi.command.WriteCommandAction;
import com.intellij.openapi.editor.Document;
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors;
import com.intellij.openapi.editor.event.DocumentEvent;
import com.intellij.openapi.editor.event.DocumentListener;
import com.intellij.psi.PsiDocumentManager;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import java.util.List;

/** Presentation never exposes old compiler types, targets, or diagnostic ranges. */
public final class NagiSemanticPresentationTest extends BasePlatformTestCase {
    private final NagiSemanticPresentation presentation = new NagiSemanticPresentation();

    @Override protected void tearDown() throws Exception {
        try { presentation.clear(); } finally { super.tearDown(); }
    }

    private Document configure(String text) {
        myFixture.configureByText("presentation.nagi", text);
        var document = myFixture.getEditor().getDocument();
        document.addDocumentListener(new DocumentListener() {
            @Override public void beforeDocumentChange(DocumentEvent event) { presentation.beforeEdit(event); }
            @Override public void documentChanged(DocumentEvent event) { presentation.afterEdit(event); }
        }, getTestRootDisposable());
        return document;
    }

    private NagiSemanticIndex.Color color(int column, int length) {
        return new NagiSemanticIndex.Color(new NagiAssistProtocol.Location(java.nio.file.Path.of("presentation.nagi").toAbsolutePath().toString(), 1, column, length),
                DefaultLanguageHighlighterColors.FUNCTION_DECLARATION);
    }

    private void edit(Document document, int start, int end, String text) {
        WriteCommandAction.runWriteCommandAction(getProject(), () -> document.replaceString(start, end, text));
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
    }

    public void testOnlyUntouchedExactTokensMoveWithUtf16Edits() {
        var document = configure("# 😀\nalpha beta\n");
        var owner = new Object();
        presentation.published(owner, document);
        var role = new NagiSemanticIndex.Color(new NagiAssistProtocol.Location(java.nio.file.Path.of("presentation.nagi").toAbsolutePath().toString(), 2, 1, 5),
                DefaultLanguageHighlighterColors.FUNCTION_DECLARATION);
        assertEquals(1, presentation.current(myFixture.getFile(), document, owner, List.of(role)).size());
        edit(document, 2, 2, "😀");
        var retained = presentation.retained(myFixture.getFile(), document);
        assertEquals(1, retained.size());
        assertEquals(document.getText().indexOf("alpha"), retained.getFirst().range().getStartOffset());
        int start = retained.getFirst().range().getStartOffset();
        edit(document, start + 2, start + 3, "x");
        assertTrue(presentation.retained(myFixture.getFile(), document).isEmpty());
        assertEquals(0, presentation.retainedRoleCount());
    }

    public void testInsertionAtEitherTokenBoundaryInvalidatesRole() {
        for (int boundary : new int[]{0, 5}) {
            var document = configure("alpha beta\n");
            Object owner = new Object();
            presentation.published(owner, document);
            presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
            edit(document, boundary, boundary, "x");
            assertTrue(presentation.retained(myFixture.getFile(), document).isEmpty());
            assertEquals(0, presentation.retainedRoleCount());
        }
    }

    public void testBoundaryWhitespaceKeepsTheExactUnchangedTokenBeforePsiCommit() {
        for (int boundary : new int[]{0, 5}) {
            for (String whitespace : new String[]{" ", "\n"}) {
                var document = configure("alpha beta\n");
                Object owner = new Object();
                presentation.published(owner, document);
                presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
                var role = ownedRole(document, 0);
                assertNotNull(role);
                WriteCommandAction.runWriteCommandAction(getProject(), () -> {
                    document.insertString(boundary, whitespace);
                    assertFalse(PsiDocumentManager.getInstance(getProject()).isCommitted(document));
                    assertTrue("boundary=" + boundary + ", text=" + whitespace.replace("\n", "Enter"), role.isValid());
                    assertEquals(document.getText().indexOf("alpha"), role.getStartOffset());
                    assertEquals(role.getStartOffset() + 5, role.getEndOffset());
                });
                PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
                assertEquals(1, presentation.retained(myFixture.getFile(), document).size());
                presentation.clear();
            }
        }
    }

    public void testBoundaryDeletionCannotRetainMergedIdentifierOrChangedLexicalKinds() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5), color(7, 4)));
        var alpha = ownedRole(document, 0);
        var beta = ownedRole(document, 6);
        assertNotNull(alpha); assertNotNull(beta);
        var joined = document;
        WriteCommandAction.runWriteCommandAction(getProject(), () -> joined.deleteString(5, 6));
        assertFalse(alpha.isValid()); assertFalse(beta.isValid());
        assertEquals(0, presentation.retainedRoleCount());
        for (String prefix : new String[]{"#", "\"", "'"}) {
            document = configure("alpha beta\n");
            presentation.published(owner, document);
            presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
            var role = ownedRole(document, 0);
            assertNotNull(role);
            var changed = document;
            WriteCommandAction.runWriteCommandAction(getProject(), () -> changed.insertString(0, prefix));
            assertFalse("a boundary edit cannot keep a token inside a comment/string", role.isValid());
            assertEquals(0, presentation.retainedRoleCount());
        }
    }

    public void testSameFocusPublicationHandsOffColorsUntilTheNextCurrentPaint() {
        var document = configure("alpha beta\n");
        Object first = new Object(), replacement = new Object();
        presentation.published(first, document);
        presentation.current(myFixture.getFile(), document, first, List.of(color(1, 5)));
        var highlighter = ownedRole(document, 0);
        assertNotNull(highlighter);

        presentation.published(replacement, document);
        assertTrue("publication does not remove the painted color", highlighter.isValid());
        assertEquals("publication must not open an unpainted gap for the same document", 1,
                presentation.retained(myFixture.getFile(), document).size());
        edit(document, document.getTextLength(), document.getTextLength(), "\n");
        assertEquals("an edit before the replacement paint still moves only the old visual role", 1,
                presentation.retained(myFixture.getFile(), document).size());
        assertTrue("the old compiler generation cannot paint", presentation.current(
                myFixture.getFile(), document, first, List.of(color(1, 5))).isEmpty());
        assertEquals(1, presentation.retainedRoleCount());

        assertTrue("an authoritative empty paint removes the old visual role", presentation.current(
                myFixture.getFile(), document, replacement, List.of()).isEmpty());
        assertEquals(0, presentation.retainedRoleCount());
        assertFalse(highlighter.isValid());
        assertTrue(presentation.retained(myFixture.getFile(), document).isEmpty());
    }

    public void testUncommittedLexicalEditCannotValidateAnOldPsiToken() {
        for (String prefix : new String[]{"#", "\"", "'"}) {
            var document = configure("alpha beta\n");
            Object owner = new Object();
            presentation.published(owner, document);
            presentation.current(myFixture.getFile(), document, owner, List.of(color(7, 4)));
            WriteCommandAction.runWriteCommandAction(getProject(), () -> {
                document.insertString(0, prefix);
                assertFalse(PsiDocumentManager.getInstance(getProject()).isCommitted(document));
                assertTrue("uncommitted old PSI must not authorize a role now inside a comment",
                        presentation.retained(myFixture.getFile(), document).isEmpty());
                assertNull("the owned color is removed before committing old PSI", ownedRole(document, 7));
            });
            PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
            assertTrue(presentation.retained(myFixture.getFile(), document).isEmpty());
            assertEquals(0, presentation.retainedRoleCount());
        }
    }

    public void testLowRoleMovesAcrossAnUncommittedUtf16EditAndAChangedPaintReplacesItsKey() {
        myFixture.configureByText("presentation.low", "# 😀\nfn alpha() {}\n");
        var document = myFixture.getEditor().getDocument();
        document.addDocumentListener(new DocumentListener() {
            @Override public void beforeDocumentChange(DocumentEvent event) { presentation.beforeEdit(event); }
            @Override public void documentChanged(DocumentEvent event) { presentation.afterEdit(event); }
        }, getTestRootDisposable());
        Object owner = new Object();
        presentation.published(owner, document);
        var point = new NagiAssistProtocol.Location(java.nio.file.Path.of("presentation.low").toAbsolutePath().toString(), 2, 4, 5);
        presentation.current(myFixture.getFile(), document, owner, List.of(new NagiSemanticIndex.Color(point,
                DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)));
        var role = ownedRole(document, document.getText().indexOf("alpha"));
        assertNotNull(role);
        WriteCommandAction.runWriteCommandAction(getProject(), () -> {
            document.insertString(2, "😀");
            assertFalse(PsiDocumentManager.getInstance(getProject()).isCommitted(document));
            assertTrue("untouched color survives even before PSI catches up", role.isValid());
            assertEquals(document.getText().indexOf("alpha"), role.getStartOffset());
        });
        PsiDocumentManager.getInstance(getProject()).commitAllDocuments();
        Object replacement = new Object();
        presentation.published(replacement, document);
        assertTrue(role.isValid());
        presentation.current(myFixture.getFile(), document, replacement, List.of(new NagiSemanticIndex.Color(point,
                DefaultLanguageHighlighterColors.FUNCTION_CALL)));
        assertFalse("new authoritative roles replace the previous style", role.isValid());
        assertTrue(java.util.Arrays.stream(com.intellij.openapi.editor.impl.DocumentMarkupModel.forDocument(
                document, getProject(), true).getAllHighlighters()).anyMatch(highlighter ->
                highlighter.getStartOffset() == document.getText().indexOf("alpha")
                        && highlighter.getTextAttributesKey() == DefaultLanguageHighlighterColors.FUNCTION_CALL));
    }

    private com.intellij.openapi.editor.markup.RangeHighlighter ownedRole(Document document, int offset) {
        return java.util.Arrays.stream(com.intellij.openapi.editor.impl.DocumentMarkupModel.forDocument(
                        document, getProject(), true).getAllHighlighters())
                .filter(highlighter -> highlighter.getStartOffset() == offset
                        && highlighter.getTextAttributesKey() == DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)
                .findFirst().orElse(null);
    }

    public void testColorKeysFollowEachEditorsSchemeAndDoNotOwnDiagnosticMarkup() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
        var role = ownedRole(document, 0);
        assertNotNull(role);
        var second = com.intellij.openapi.editor.EditorFactory.getInstance().createEditor(document, getProject());
        var markup = com.intellij.openapi.editor.impl.DocumentMarkupModel.forDocument(document, getProject(), true);
        var diagnostic = markup.addRangeHighlighter(0, 5, com.intellij.openapi.editor.markup.HighlighterLayer.ERROR,
                new com.intellij.openapi.editor.markup.TextAttributes(null, null, java.awt.Color.RED,
                        com.intellij.openapi.editor.markup.EffectType.WAVE_UNDERSCORE, 0),
                com.intellij.openapi.editor.markup.HighlighterTargetArea.EXACT_RANGE);
        try {
            var scheme = new com.intellij.openapi.editor.colors.impl.EditorColorsSchemeImpl(second.getColorsScheme());
            var attributes = new com.intellij.openapi.editor.markup.TextAttributes(java.awt.Color.GREEN, null, null, null, 0);
            scheme.setAttributes(DefaultLanguageHighlighterColors.FUNCTION_DECLARATION, attributes);
            ((com.intellij.openapi.editor.ex.EditorEx) second).setColorsScheme(scheme);
            assertEquals(attributes, role.getTextAttributes(second.getColorsScheme()));
            assertSame("the layer keeps a scheme key, not resolved RGB attributes",
                    DefaultLanguageHighlighterColors.FUNCTION_DECLARATION, role.getTextAttributesKey());
            assertTrue("diagnostic effects keep their higher IDE layer", diagnostic.getLayer() > role.getLayer());
            assertNull(role.getErrorStripeTooltip());
            presentation.clear();
            assertFalse(role.isValid());
            assertTrue("clearing color ownership cannot remove diagnostic markup", diagnostic.isValid());
        } finally {
            diagnostic.dispose();
            com.intellij.openapi.editor.EditorFactory.getInstance().releaseEditor(second);
        }
    }

    public void testDependencyEditAndGenerationReplacementDiscardAllMarkers() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
        var dependency = com.intellij.openapi.editor.EditorFactory.getInstance().createDocument("dependency");
        dependency.addDocumentListener(new DocumentListener() {
            @Override public void beforeDocumentChange(DocumentEvent event) { presentation.beforeEdit(event); }
        }, getTestRootDisposable());
        edit(dependency, 0, 0, "x");
        assertEquals(0, presentation.retainedRoleCount());
        assertTrue(presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5))).isEmpty());
        Object replacement = new Object();
        presentation.published(replacement, document);
        presentation.current(myFixture.getFile(), document, replacement, List.of(color(1, 5)));
        presentation.clear();
        assertEquals(0, presentation.retainedRoleCount());
        assertTrue(presentation.retained(myFixture.getFile(), document).isEmpty());
        assertTrue(presentation.current(myFixture.getFile(), document, replacement, List.of(color(1, 5))).isEmpty());
    }

    public void testCancelledPaintingReleasesPartiallyAllocatedMarkers() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        var indicator = new com.intellij.openapi.progress.EmptyProgressIndicator();
        List<NagiSemanticIndex.Color> colors = new java.util.AbstractList<>() {
            @Override public int size() { return 2; }
            @Override public NagiSemanticIndex.Color get(int index) {
                if (index == 1) indicator.cancel();
                return index == 0 ? color(1, 5) : color(7, 4);
            }
        };
        try {
            com.intellij.openapi.progress.ProgressManager.getInstance().runProcess(
                    () -> presentation.current(myFixture.getFile(), document, owner, colors), indicator);
            fail("a cancelled annotation pass must remain cancelled");
        } catch (com.intellij.openapi.progress.ProcessCanceledException expected) { }
        assertEquals("cancelled paint cannot leak the first allocated marker", 0, presentation.retainedRoleCount());
        assertTrue(presentation.retained(myFixture.getFile(), document).isEmpty());
    }

    public void testCancelledReplacementPreservesPreviouslyPaintedMarkersAndColor() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
        var role = ownedRole(document, 0);
        assertNotNull(role);
        var indicator = new com.intellij.openapi.progress.EmptyProgressIndicator();
        List<NagiSemanticIndex.Color> colors = new java.util.AbstractList<>() {
            @Override public int size() { return 2; }
            @Override public NagiSemanticIndex.Color get(int index) {
                if (index == 1) indicator.cancel();
                return index == 0 ? color(7, 4) : color(1, 5);
            }
        };
        try {
            com.intellij.openapi.progress.ProgressManager.getInstance().runProcess(
                    () -> presentation.current(myFixture.getFile(), document, owner, colors), indicator);
            fail("replacement cancellation must propagate");
        } catch (com.intellij.openapi.progress.ProcessCanceledException expected) { }
        assertEquals("cancellation cannot dispose the previous color's validation marker", 1,
                presentation.retainedRoleCount());
        assertEquals(1, presentation.retained(myFixture.getFile(), document).size());
        assertTrue(role.isValid());
        assertSame("an aborted replacement never removes the visible color", role, ownedRole(document, 0));
        assertNull("no half-painted replacement remains", ownedRole(document, 6));
    }

    public void testStorageIsBoundedAndCannotRetainPartialOrOversizeTokens() throws Exception {
        var document = configure("alpha " + "z".repeat(257) + "\n");
        Object owner = new Object();
        presentation.published(owner, document);
        var invalid = List.of(color(2, 3), color(7, 257), color(Integer.MAX_VALUE, 5));
        assertEquals(1, presentation.current(myFixture.getFile(), document, owner, invalid).size());
        assertEquals(0, presentation.retainedRoleCount());
        // Enter immediately below the allocation cap, avoiding 20,000 duplicate
        // same-offset platform markers (a pathological interval-tree fixture).
        var count = NagiSemanticPresentation.class.getDeclaredField("roleCount");
        count.setAccessible(true);
        count.setInt(presentation, NagiSemanticPresentation.MAX_ROLES - 1);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5), color(1, 5)));
        assertEquals(NagiSemanticPresentation.MAX_ROLES, presentation.retainedRoleCount());
        assertEquals(1, presentation.retained(myFixture.getFile(), document).size());
        Object replacement = new Object();
        presentation.published(replacement, document);
        assertEquals("publication keeps the bounded visual handoff until paint", NagiSemanticPresentation.MAX_ROLES,
                presentation.retainedRoleCount());
        assertTrue(presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5))).isEmpty());
        // Leave the artificial near-cap counter fixture: one real marker was allocated.
        count.setInt(presentation, 1);
        presentation.current(myFixture.getFile(), document, replacement, List.of());
        assertEquals(0, presentation.retainedRoleCount());
    }

    public void testMarkerAllocationFailureOrCancellationKeepsThePreviousStore() {
        for (boolean cancellation : new boolean[]{false, true}) {
            var document = configure("alpha beta\n");
            var calls = new java.util.concurrent.atomic.AtomicInteger();
            var temporary = new java.util.concurrent.atomic.AtomicReference<com.intellij.openapi.editor.RangeMarker>();
            RuntimeException injected = cancellation ? new com.intellij.openapi.progress.ProcessCanceledException()
                    : new IllegalStateException("injected marker allocation failure");
            var tested = new NagiSemanticPresentation((doc, range) -> {
                if (calls.incrementAndGet() == 3) throw injected;
                var marker = doc.createRangeMarker(range);
                if (calls.get() == 2) temporary.set(marker);
                return marker;
            });
            try {
                Object owner = new Object();
                tested.published(owner, document);
                tested.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
                var original = ownedRole(document, 0);
                try {
                    tested.current(myFixture.getFile(), document, owner, List.of(color(7, 4), color(1, 5)));
                    fail("marker factory failure must propagate");
                } catch (RuntimeException failure) { assertSame(injected, failure); }
                assertNotNull(temporary.get());
                assertFalse("only the newly prepared marker is disposed", temporary.get().isValid());
                assertEquals(1, tested.retainedRoleCount());
                assertEquals(1, tested.retained(myFixture.getFile(), document).size());
                assertSame(original, ownedRole(document, 0));
                assertTrue(original.isValid());
            } finally { tested.clear(); }
        }
    }

    public void testHardClearDuringMarkerPreparationCannotResurrectAColor() {
        var document = configure("alpha beta\n");
        var calls = new java.util.concurrent.atomic.AtomicInteger();
        var self = new java.util.concurrent.atomic.AtomicReference<NagiSemanticPresentation>();
        var temporary = new java.util.concurrent.atomic.AtomicReference<com.intellij.openapi.editor.RangeMarker>();
        var tested = new NagiSemanticPresentation((doc, range) -> {
            if (calls.incrementAndGet() == 2) self.get().clear();
            var marker = doc.createRangeMarker(range);
            if (calls.get() == 2) temporary.set(marker);
            return marker;
        });
        self.set(tested);
        try {
            Object owner = new Object();
            tested.published(owner, document);
            tested.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
            assertTrue(tested.current(myFixture.getFile(), document, owner, List.of(color(7, 4))).isEmpty());
            assertFalse(temporary.get().isValid());
            assertEquals(0, tested.retainedRoleCount());
            assertNull(ownedRole(document, 0)); assertNull(ownedRole(document, 6));
        } finally { tested.clear(); }
    }

    public void testColorPreparationCancellationKeepsThePreviousStoreAndColor() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
        var original = ownedRole(document, 0);
        var indicator = new com.intellij.openapi.progress.EmptyProgressIndicator();
        var temporary = new java.util.concurrent.atomic.AtomicReference<com.intellij.openapi.editor.markup.RangeHighlighter>();
        ((com.intellij.openapi.editor.ex.MarkupModelEx) com.intellij.openapi.editor.impl.DocumentMarkupModel.forDocument(
                document, getProject(), true)).addMarkupModelListener(getTestRootDisposable(),
                new com.intellij.openapi.editor.impl.event.MarkupModelListener() {
                    @Override public void afterAdded(com.intellij.openapi.editor.ex.RangeHighlighterEx highlighter) {
                        if (highlighter.getStartOffset() == 6) { temporary.set(highlighter); indicator.cancel(); }
                    }
                });
        try {
            com.intellij.openapi.progress.ProgressManager.getInstance().runProcess(() -> presentation.current(
                    myFixture.getFile(), document, owner, List.of(color(7, 4), color(1, 5))), indicator);
            fail("color-stage cancellation must propagate");
        } catch (com.intellij.openapi.progress.ProcessCanceledException expected) { }
        assertNotNull(temporary.get()); assertFalse(temporary.get().isValid());
        assertEquals(1, presentation.retainedRoleCount());
        assertEquals(1, presentation.retained(myFixture.getFile(), document).size());
        assertSame(original, ownedRole(document, 0)); assertTrue(original.isValid());
    }

    public void testHardClearDuringColorPreparationCannotResurrectAColor() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
        var temporary = new java.util.concurrent.atomic.AtomicReference<com.intellij.openapi.editor.markup.RangeHighlighter>();
        ((com.intellij.openapi.editor.ex.MarkupModelEx) com.intellij.openapi.editor.impl.DocumentMarkupModel.forDocument(
                document, getProject(), true)).addMarkupModelListener(getTestRootDisposable(),
                new com.intellij.openapi.editor.impl.event.MarkupModelListener() {
                    @Override public void afterAdded(com.intellij.openapi.editor.ex.RangeHighlighterEx highlighter) {
                        if (highlighter.getStartOffset() == 6) { temporary.set(highlighter); presentation.clear(); }
                    }
                });
        assertTrue(presentation.current(myFixture.getFile(), document, owner, List.of(color(7, 4))).isEmpty());
        assertNotNull(temporary.get()); assertFalse(temporary.get().isValid());
        assertEquals(0, presentation.retainedRoleCount());
        assertNull(ownedRole(document, 0)); assertNull(ownedRole(document, 6));
    }

    public void testDirectColorAddedListenerPceCannotLeakTemporaryOwnership() {
        throwingColorAddedListener(new com.intellij.openapi.progress.ProcessCanceledException());
    }

    public void testDirectColorAddedListenerRuntimeFailureCannotLeakTemporaryOwnership() {
        throwingColorAddedListener(new IllegalStateException("injected afterAdded failure"));
    }

    private void throwingColorAddedListener(RuntimeException injected) {
        for (boolean seeded : new boolean[]{true, false}) {
            var document = configure("alpha beta\n");
            Object owner = new Object();
            presentation.published(owner, document);
            if (seeded) presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
            var original = ownedRole(document, 0);
            if (seeded) assertNotNull(original);
            var attempted = new java.util.concurrent.atomic.AtomicReference<com.intellij.openapi.editor.markup.RangeHighlighter>();
            var markup = (com.intellij.openapi.editor.ex.MarkupModelEx) com.intellij.openapi.editor.impl.DocumentMarkupModel
                    .forDocument(document, getProject(), true);
            var listenerLifetime = com.intellij.openapi.util.Disposer.newDisposable("direct afterAdded failure case");
            markup.addMarkupModelListener(listenerLifetime, new com.intellij.openapi.editor.impl.event.MarkupModelListener() {
                @Override public void afterAdded(com.intellij.openapi.editor.ex.RangeHighlighterEx highlighter) {
                    if (highlighter.getStartOffset() == 6) {
                        attempted.set(highlighter);
                        assertTrue("the SDK registers the range before invoking afterAdded", markup.containsHighlighter(highlighter));
                        throw injected;
                    }
                }
            });
            try {
                try {
                    presentation.current(myFixture.getFile(), document, owner, List.of(color(7, 4)));
                    fail("the listener's direct failure must propagate");
                } catch (RuntimeException failure) { assertSame(injected, failure); }
                assertNotNull("this iteration's listener must observe its own addition; seeded=" + seeded, attempted.get());
                assertEquals(seeded ? 1 : 0, presentation.retainedRoleCount());
                assertEquals(seeded ? 1 : 0, presentation.retained(myFixture.getFile(), document).size());
                if (seeded) {
                    assertTrue(original.isValid());
                    assertSame("a failed replacement cannot remove the original color", original, ownedRole(document, 0));
                }
                assertFalse("a failure before addRangeHighlighter returns cannot orphan its registered range", attempted.get().isValid());
                assertFalse(markup.containsHighlighter(attempted.get()));
                assertNull(ownedRole(document, 6));
                presentation.clear();
                assertEquals(0, presentation.retainedRoleCount());
                assertNull(ownedRole(document, 0)); assertNull(ownedRole(document, 6));
                assertFalse(attempted.get().isValid());
            } finally {
                com.intellij.openapi.util.Disposer.dispose(listenerLifetime);
                presentation.clear();
                // Preserve fixture isolation even when the pre-fix leak assertion is RED.
                if (attempted.get() != null && attempted.get().isValid()) attempted.get().dispose();
            }
        }
    }

    public void testPartialPaintOverlaysNewRolesAndKeepsOnlyValidatedUncoveredColors() {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5), color(7, 4)));
        Object partial = new Object();
        presentation.published(partial, document);
        var changed = new NagiSemanticIndex.Color(color(1, 5).location(), DefaultLanguageHighlighterColors.FUNCTION_CALL);
        var paints = presentation.partial(myFixture.getFile(), document, partial, List.of(changed));
        assertEquals(2, paints.size());
        assertTrue(paints.stream().anyMatch(p -> p.range().getStartOffset() == 0
                && p.attributes() == DefaultLanguageHighlighterColors.FUNCTION_CALL));
        assertTrue(paints.stream().anyMatch(p -> p.range().getStartOffset() == 6
                && p.attributes() == DefaultLanguageHighlighterColors.FUNCTION_DECLARATION));
        assertEquals(2, presentation.partial(myFixture.getFile(), document, partial, List.of()).size());
        edit(document, 6, 7, "x");
        assertEquals("a changed token cannot inherit a retained role", 1,
                presentation.partial(myFixture.getFile(), document, partial, List.of()).size());
        presentation.current(myFixture.getFile(), document, partial, List.of());
        assertEquals("an explicit complete empty paint still clears", 0, presentation.retainedRoleCount());
    }

    public void testPartialPaintPrioritizesProvidedMarkersWithinTheExistingLimit() throws Exception {
        var document = configure("alpha beta\n");
        Object owner = new Object();
        presentation.published(owner, document);
        presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5)));
        var count = NagiSemanticPresentation.class.getDeclaredField("roleCount");
        count.setAccessible(true);
        count.setInt(presentation, NagiSemanticPresentation.MAX_ROLES);
        var provided = new NagiSemanticIndex.Color(color(7, 4).location(), DefaultLanguageHighlighterColors.FUNCTION_CALL);
        assertEquals(2, presentation.partial(myFixture.getFile(), document, owner, List.of(provided)).size());
        assertEquals(NagiSemanticPresentation.MAX_ROLES, presentation.retainedRoleCount());
        var retained = presentation.retained(myFixture.getFile(), document);
        assertEquals(1, retained.size());
        assertEquals(6, retained.getFirst().range().getStartOffset());
        assertSame(DefaultLanguageHighlighterColors.FUNCTION_CALL, retained.getFirst().attributes());
    }

    public void testUnseededMarkerPreparationFailureCannotLeakMarkersOrPublishColors() {
        var document = configure("alpha beta\n");
        var created = new java.util.concurrent.atomic.AtomicReference<com.intellij.openapi.editor.RangeMarker>();
        var tested = new NagiSemanticPresentation((doc, range) -> {
            if (created.get() != null) throw new com.intellij.openapi.progress.ProcessCanceledException();
            var marker = doc.createRangeMarker(range); created.set(marker); return marker;
        });
        try {
            Object owner = new Object(); tested.published(owner, document);
            try {
                tested.current(myFixture.getFile(), document, owner, List.of(color(1, 5), color(7, 4)));
                fail("marker-stage cancellation must propagate");
            } catch (com.intellij.openapi.progress.ProcessCanceledException expected) { }
            assertNotNull(created.get()); assertFalse(created.get().isValid());
            assertEquals(0, tested.retainedRoleCount());
            assertTrue(tested.retained(myFixture.getFile(), document).isEmpty());
            assertNull(ownedRole(document, 0)); assertNull(ownedRole(document, 6));
        } finally { tested.clear(); }
    }
}
