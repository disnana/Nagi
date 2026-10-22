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
        presentation.published(new Object(), document);
        assertEquals(0, presentation.retainedRoleCount());
        assertTrue(presentation.current(myFixture.getFile(), document, owner, List.of(color(1, 5))).isEmpty());
    }
}
