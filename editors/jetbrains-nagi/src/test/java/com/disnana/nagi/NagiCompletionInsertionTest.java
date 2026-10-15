package com.disnana.nagi;

import com.intellij.codeInsight.CodeInsightSettings;
import com.intellij.codeInsight.lookup.Lookup;
import com.intellij.codeInsight.lookup.LookupElement;
import com.intellij.ide.trustedProjects.TrustedProjects;
import com.intellij.openapi.vfs.LocalFileSystem;
import com.intellij.testFramework.ServiceContainerUtil;
import com.intellij.testFramework.fixtures.BasePlatformTestCase;
import com.intellij.util.ui.UIUtil;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

/** The registered contributor is accepted through an actual editor lookup. */
public class NagiCompletionInsertionTest extends BasePlatformTestCase {
    private Path root;
    private NagiAssistService service;
    private boolean originalTrust;
    private String originalCompiler;
    private int originalTimeout;
    private final AtomicReference<NagiAssistProtocol.CompletionItem> offered = new AtomicReference<>();
    private final AtomicReference<String> sourcePath = new AtomicReference<>();

    @Override protected void setUp() throws Exception {
        super.setUp();
        Path base = Path.of(getProject().getBasePath());
        Files.createDirectories(base);
        root = Files.createTempDirectory(base, "nagi completion insertion ");
        originalTrust = TrustedProjects.isProjectTrusted(getProject());
        originalCompiler = NagiSettings.getInstance().getState().compilerPath;
        originalTimeout = NagiSettings.getInstance().getState().checkTimeoutSeconds;
        TrustedProjects.setProjectTrusted(getProject(), true);
        NagiSettings.update(Path.of(System.getProperty("java.home"), "bin", "java").toString(), 5);
        service = new NagiAssistService(getProject(), (plan, input, timeout, cancellation, guard) ->
                new NagiAssistProtocol.Response("editor-partial", true, false, false, "names", List.of(),
                        List.of(offered.get()), List.of(), List.of(sourcePath.get())));
        ServiceContainerUtil.replaceService(getProject(), NagiAssistService.class, service, getTestRootDisposable());
    }

    @Override protected void tearDown() throws Exception {
        try {
            if (service != null) com.intellij.openapi.util.Disposer.dispose(service);
            NagiSettings.update(originalCompiler, originalTimeout);
            TrustedProjects.setProjectTrusted(getProject(), originalTrust);
            if (root != null) try (var files = Files.walk(root)) {
                for (Path file : files.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
        } finally { super.tearDown(); }
    }

    private void open(String name, String markedText, String kind) throws Exception {
        int caret = markedText.indexOf("<caret>");
        assertTrue(caret >= 0);
        String text = markedText.replace("<caret>", "");
        Path path = root.resolve(name);
        sourcePath.set(path.toString());
        offered.set(new NagiAssistProtocol.CompletionItem("helperAction", kind, "fn(i64) -> i64",
                "helperAction(value: i64) -> i64", null, false, "read"));
        Files.writeString(path, text);
        var virtualFile = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(path);
        assertNotNull(virtualFile);
        myFixture.configureFromExistingVirtualFile(virtualFile);
        myFixture.getEditor().getCaretModel().moveToOffset(caret);
        service.requestCompletion(myFixture.getFile(), myFixture.getEditor());
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
        while (service.fresh(myFixture.getFile(), myFixture.getEditor()) == null && System.nanoTime() < deadline) {
            UIUtil.dispatchAllInvocationEvents();
            Thread.sleep(5);
        }
        assertNotNull(service.state() + ": " + service.statusMessage(),
                service.fresh(myFixture.getFile(), myFixture.getEditor()));
    }

    private void accept(char selection) {
        LookupElement[] variants = myFixture.completeBasic();
        assertNotNull(variants);
        LookupElement candidate = Arrays.stream(variants)
                .filter(item -> item.getLookupString().equals("helperAction")).findFirst().orElse(null);
        assertNotNull(candidate);
        assertNotNull(myFixture.getLookup());
        myFixture.getLookup().setCurrentItem(candidate);
        myFixture.finishLookup(selection);
    }

    public void testFunctionReferenceDoesNotBecomeCallInHighOrLow() throws Exception {
        open("reference.nagi", "def main():\n    callback = hel<caret>", "function");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("def main():\n    callback = helperAction", myFixture.getEditor().getDocument().getText());

        open("reference.low", "fn main() { let callback = hel<caret>; }", "function");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("fn main() { let callback = helperAction; }", myFixture.getEditor().getDocument().getText());
    }

    public void testExistingArgumentListsArePreserved() throws Exception {
        open("arguments.nagi", "def main():\n    consume(hel<caret>)", "function");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("def main():\n    consume(helperAction)", myFixture.getEditor().getDocument().getText());

        open("spaced.nagi", "def main():\n    hel<caret> (1)", "function");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("def main():\n    helperAction (1)", myFixture.getEditor().getDocument().getText());

        open("next-line.nagi", "def main():\n    hel<caret>\n    (1)", "function");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("def main():\n    helperAction\n    (1)", myFixture.getEditor().getDocument().getText());

        open("declaration.nagi", "def hel<caret>():\n    return 1", "function");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("def helperAction():\n    return 1", myFixture.getEditor().getDocument().getText());
    }

    public void testExplicitOpenParenthesisHonorsPairSetting() throws Exception {
        open("call.nagi", "def main():\n    hel<caret>", "function");
        accept('(');
        assertEquals("def main():\n    helperAction()", myFixture.getEditor().getDocument().getText());

        open("call.low", "fn main() { hel<caret>; }", "function");
        accept('(');
        assertEquals("fn main() { helperAction(); }", myFixture.getEditor().getDocument().getText());

        open("existing-spaced.nagi", "def main():\n    hel<caret> (1)", "function");
        accept('(');
        assertEquals("def main():\n    helperAction (1)", myFixture.getEditor().getDocument().getText());
        open("existing-next-line.nagi", "def main():\n    hel<caret>\n    (1)", "function");
        accept('(');
        assertEquals("def main():\n    helperAction\n    (1)", myFixture.getEditor().getDocument().getText());

        var smartKeys = CodeInsightSettings.getInstance();
        boolean previous = smartKeys.AUTOINSERT_PAIR_BRACKET;
        try {
            smartKeys.AUTOINSERT_PAIR_BRACKET = false;
            open("call-no-pair.nagi", "def main():\n    hel<caret>", "function");
            accept('(');
            assertEquals("def main():\n    helperAction(", myFixture.getEditor().getDocument().getText());
        } finally { smartKeys.AUTOINSERT_PAIR_BRACKET = previous; }
    }

    public void testFunctionValuedLocalStaysAName() throws Exception {
        open("local.nagi", "def main():\n    callback = hel<caret>", "local");
        accept(Lookup.NORMAL_SELECT_CHAR);
        assertEquals("def main():\n    callback = helperAction", myFixture.getEditor().getDocument().getText());
    }

    public void testNoNagiCandidateInsideCommentOrString() throws Exception {
        open("comment.nagi", "def main():\n    # hel<caret>", "function");
        assertNoNagiCandidate();
        open("string.nagi", "def main():\n    value = \"hel<caret>\"", "function");
        assertNoNagiCandidate();
    }

    private void assertNoNagiCandidate() {
        LookupElement[] variants = myFixture.completeBasic();
        assertTrue(variants == null || Arrays.stream(variants)
                .noneMatch(item -> item.getLookupString().equals("helperAction")));
    }
}
