package com.disnana.nagi;

import com.intellij.codeInsight.lookup.Lookup;
import com.intellij.codeInsight.lookup.LookupElement;
import com.intellij.codeInsight.lookup.LookupEvent;
import com.intellij.codeInsight.lookup.LookupListener;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.testFramework.ServiceContainerUtil;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.testFramework.PlatformTestUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.atomic.AtomicBoolean;

/** Compiler-ranged import insertions through the registered platform contributor. */
public final class NagiAssistImportInsertionTest extends BasePlatformTestCase {
    private Path root;
    private NagiAssistService service;
    private boolean originalTrust;
    private String originalCompiler;
    private int originalTimeout;
    private final AtomicReference<NagiAssistProtocol.Response> response = new AtomicReference<>();

    @Override protected void setUp() throws Exception {
        super.setUp();
        // The fixture's default policy rewrites input bytes with normalized LF.
        // Preserve the physical CRLF input and let FileDocumentManager save it.
        ((com.intellij.testFramework.fixtures.impl.CodeInsightTestFixtureImpl) myFixture)
                .setSelectionAndCaretMarkupApplyPolicy(
                        com.intellij.testFramework.fixtures.impl.CodeInsightTestFixtureImpl
                                .SelectionAndCaretMarkupApplyPolicy.UPDATE_DOCUMENT_AND_LEAVE_IT_DIRTY);
        Path base = Path.of(getProject().getBasePath());
        Files.createDirectories(base);
        root = Files.createTempDirectory(base, "nagi import completion ");
        originalTrust = TrustedProjects.isProjectTrusted(getProject());
        originalCompiler = NagiSettings.getInstance().getState().compilerPath;
        originalTimeout = NagiSettings.getInstance().getState().checkTimeoutSeconds;
        TrustedProjects.setProjectTrusted(getProject(), true);
        NagiSettings.update(Path.of(System.getProperty("java.home"), "bin", "java").toString(), 5);
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) -> {
            String request = new String(input, java.nio.charset.StandardCharsets.UTF_8);
            return request.contains("\"query\"") ? response.get()
                    : new NagiAssistProtocol.Response("none", false, false, false, "names", List.of(),
                            List.of(), List.of(), List.of());
        });
        ServiceContainerUtil.replaceService(getProject(), NagiAssistService.class, service, getTestRootDisposable());
    }

    @Override protected void tearDown() throws Exception {
        try {
            if (service != null) com.intellij.openapi.util.Disposer.dispose(service);
            NagiSettings.update(originalCompiler, originalTimeout);
            TrustedProjects.setProjectTrusted(getProject(), originalTrust);
            NagiSaveBarrier.awaitDiskWrites();
            if (root != null) try (var files = Files.walk(root)) {
                for (Path file : files.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
        } catch (Throwable failure) {
            addSuppressedException(failure);
        } finally {
            super.tearDown();
        }
    }

    private void open(String name, String text, int caret, int replacementStart, int replacementEnd,
                      String context, String candidate, String kind, boolean reportDirectory) throws Exception {
        Path path = root.resolve(name);
        Files.writeString(path, text);
        var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(virtualFile);
        String filePath = virtualFile.getPath();
        NagiAssistProtocol.Location replacement = location(filePath, text, replacementStart, replacementEnd);
        List<String> directories = reportDirectory ? List.of(root.toString()) : List.of();
        var importContext = new NagiAssistProtocol.ImportContext(context, replacement, directories, false);
        var completion = new NagiAssistProtocol.CompletionItem(candidate, kind, null, null, null, false, "read");
        response.set(new NagiAssistProtocol.Response("editor-partial", false, false, false, "names", List.of(),
                List.of(completion), List.of(), List.of(filePath), List.of(), List.of(), List.of(), importContext));

        myFixture.configureFromExistingVirtualFile(virtualFile);
        assertEquals(text.replace("\r\n", "\n"), myFixture.getEditor().getDocument().getText());
        int documentCaret = text.substring(0, caret).replace("\r\n", "\n").length();
        myFixture.getEditor().getCaretModel().moveToOffset(documentCaret);
        service.requestCompletion(myFixture.getFile(), myFixture.getEditor());
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
        while (service.fresh(myFixture.getFile(), myFixture.getEditor()) == null && System.nanoTime() < deadline) {
            // Let the IDE dispatcher release implicit write intent for background VFS events.
            PlatformTestUtil.dispatchAllInvocationEventsInIdeEventQueue();
            Thread.sleep(5);
        }
        assertNotNull(service.state() + ": " + service.statusMessage(),
                service.fresh(myFixture.getFile(), myFixture.getEditor()));
    }

    private static NagiAssistProtocol.Location location(String file, String text, int start, int end) {
        int line = 1;
        int lineStart = 0;
        for (int index = 0; index < start; index++) {
            if (text.charAt(index) == '\n') { line++; lineStart = index + 1; }
        }
        return new NagiAssistProtocol.Location(file, line, start - lineStart + 1, end - start);
    }

    private void accept(String name, char selection) {
        LookupElement[] variants = myFixture.completeBasic();
        assertNotNull(variants);
        LookupElement candidate = Arrays.stream(variants)
                .filter(item -> item.getLookupString().equals(name)).findFirst().orElse(null);
        assertNotNull("missing import candidate " + name, candidate);
        assertNotNull(myFixture.getLookup());
        myFixture.getLookup().setCurrentItem(candidate);
        myFixture.finishLookup(selection);
    }

    public void testSourceCompletionReplacesOnlyReportedTokenIncludingQualifiedStdName() throws Exception {
        Files.writeString(root.resolve("utils.nagi"), "def utility() -> i64:\n    return 1\n");
        String text = "import ut\n";
        open("main.nagi", text, text.indexOf("ut") + 2, text.indexOf("ut"), text.indexOf("ut") + 2,
                "import-source", "utils", "module", true);
        accept("utils", Lookup.NORMAL_SELECT_CHAR);
        assertEquals("import utils\n", myFixture.getEditor().getDocument().getText());

        text = "import std.f\r\n";
        int start = text.indexOf("std.f");
        open("qualified.nagi", text, start + "std.f".length(), start, start + "std.f".length(),
                "import-source", "std.fs", "module", true);
        accept("std.fs", Lookup.NORMAL_SELECT_CHAR);
        assertEquals("import std.fs\n", myFixture.getEditor().getDocument().getText());
        FileDocumentManager.getInstance().saveDocument(myFixture.getEditor().getDocument());
        NagiSaveBarrier.awaitDiskWrites();
        assertEquals("import std.fs\r\n", Files.readString(root.resolve("qualified.nagi")));
    }

    public void testFromNameInsertionPreservesAliasAndFollowingSelections() throws Exception {
        String text = "from helper import buildThing as renamed, keep\n";
        int start = text.indexOf("buildThing");
        open("alias.nagi", text, start + 2, start, start + "buildThing".length(),
                "import-names", "buildAction", "function", false);
        accept("buildAction", Lookup.NORMAL_SELECT_CHAR);
        assertEquals("from helper import buildAction as renamed, keep\n", myFixture.getEditor().getDocument().getText());

        text = "from helper import old, va, last\n";
        start = text.indexOf("va,");
        open("multiple.nagi", text, start + 2, start, start + 2,
                "import-names", "value", "class", false);
        accept("value", Lookup.NORMAL_SELECT_CHAR);
        assertEquals("from helper import old, value, last\n", myFixture.getEditor().getDocument().getText());

        text = "from helper import va  # keep this comment\n";
        start = text.indexOf("va");
        open("comment.nagi", text, start + 1, start, start + "va".length(),
                "import-names", "value", "class", false);
        accept("value", Lookup.NORMAL_SELECT_CHAR);
        assertEquals("from helper import value  # keep this comment\n", myFixture.getEditor().getDocument().getText());
    }

    public void testEmptyUtf16CrlfSelectionInsertsFunctionNameWithoutCallSyntax() throws Exception {
        String text = "# 🐈\r\nfrom helper import \r\n";
        int caret = text.indexOf("\r\n", text.indexOf("from helper"));
        open("empty.nagi", text, caret, caret, caret, "import-names", "build", "function", false);
        accept("build", '(');
        assertEquals("# 🐈\nfrom helper import build\n", myFixture.getEditor().getDocument().getText());
        FileDocumentManager.getInstance().saveDocument(myFixture.getEditor().getDocument());
        NagiSaveBarrier.awaitDiskWrites();
        assertEquals("# 🐈\r\nfrom helper import build\r\n", Files.readString(root.resolve("empty.nagi")));
    }

    public void testStaleSourceLookupCannotInsertAfterDirectoryChanges() throws Exception {
        String text = "import ut\n";
        open("stale.nagi", text, text.indexOf("ut") + 2, text.indexOf("ut"), text.indexOf("ut") + 2,
                "import-source", "utils", "module", true);
        LookupElement candidate = Arrays.stream(myFixture.completeBasic())
                .filter(item -> item.getLookupString().equals("utils")).findFirst().orElse(null);
        assertNotNull(candidate);
        var lookup = myFixture.getLookup();
        assertNotNull(lookup);
        lookup.setCurrentItem(candidate);
        AtomicBoolean canceled = new AtomicBoolean();
        lookup.addLookupListener(new LookupListener() {
            @Override public void lookupCanceled(LookupEvent event) {
                canceled.set(true);
                event.getLookup().removeLookupListener(this);
            }
        });
        var published = service.fresh(myFixture.getFile(), myFixture.getEditor());
        assertNotNull(published);

        Files.writeString(root.resolve("late_module.nagi"), "def late() -> i64:\n    return 1\n");
        CountDownLatch refreshed = new CountDownLatch(1);
        var parent = LocalFileSystem.getInstance().findFileByPath(root.toString());
        assertNotNull(parent);
        com.intellij.openapi.vfs.VfsUtil.markDirty(false, false, parent);
        parent.refresh(true, false, refreshed::countDown);
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
        while ((refreshed.getCount() != 0 || service.isCurrent(myFixture.getFile(), published))
                && System.nanoTime() < deadline) {
            PlatformTestUtil.dispatchAllInvocationEventsInIdeEventQueue();
            Thread.sleep(5);
        }
        assertEquals("targeted parent refresh must finish before selection", 0L, refreshed.getCount());
        assertFalse("observed directory change invalidates the published response",
                service.isCurrent(myFixture.getFile(), published));
        if (myFixture.getLookup() == lookup && !canceled.get()) {
            myFixture.finishLookup(Lookup.NORMAL_SELECT_CHAR);
        } else {
            assertTrue("the stale lookup must have been canceled rather than retained for selection",
                    canceled.get());
        }
        assertEquals(text, myFixture.getEditor().getDocument().getText());
    }
}
