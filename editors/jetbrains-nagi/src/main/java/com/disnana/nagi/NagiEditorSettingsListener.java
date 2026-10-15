package com.disnana.nagi;

import com.intellij.application.options.CodeStyle;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.application.ModalityState;
import com.intellij.openapi.application.ReadAction;
import com.intellij.openapi.editor.Editor;
import com.intellij.openapi.editor.EditorFactory;
import com.intellij.openapi.editor.event.EditorFactoryEvent;
import com.intellij.openapi.editor.event.EditorFactoryListener;
import com.intellij.openapi.fileEditor.FileDocumentManager;
import com.intellij.openapi.project.Project;
import com.intellij.openapi.util.Key;
import com.intellij.openapi.vfs.VirtualFile;
import com.intellij.openapi.vfs.newvfs.BulkFileListener;
import com.intellij.openapi.vfs.newvfs.events.VFileEvent;
import com.intellij.openapi.vfs.newvfs.events.VFilePropertyChangeEvent;
import com.intellij.psi.codeStyle.CodeStyleSettingsChangeEvent;
import com.intellij.psi.codeStyle.CodeStyleSettingsListener;
import com.intellij.util.concurrency.AppExecutorUtil;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import org.jetbrains.annotations.NotNull;

/** Keep Nagi editor actions space-only while retaining the configured indent width. */
public final class NagiEditorSettingsListener
        implements EditorFactoryListener, BulkFileListener, CodeStyleSettingsListener {
    // The three listener registrations have separate instances. Weak keys avoid retaining closed editors.
    private static final Set<Editor> TOUCHED = Collections.synchronizedSet(Collections.newSetFromMap(new WeakHashMap<>()));
    private static final Key<AtomicInteger> GENERATION = Key.create("nagi.space.only.generation");

    @Override public void editorCreated(@NotNull EditorFactoryEvent event) {
        sync(event.getEditor());
    }

    @Override public void editorReleased(@NotNull EditorFactoryEvent event) {
        TOUCHED.remove(event.getEditor());
    }

    @Override public void after(@NotNull List<? extends VFileEvent> events) {
        Set<VirtualFile> renamed = new HashSet<>();
        Set<VirtualFile> changedConfigs = new HashSet<>();
        for (VFileEvent event : events) {
            VirtualFile file = event.getFile();
            if (event instanceof VFilePropertyChangeEvent change
                    && VirtualFile.PROP_NAME.equals(change.getPropertyName())) {
                if (file != null) renamed.add(file);
                if (".editorconfig".equals(change.getOldValue()) && file != null) changedConfigs.add(file);
            }
            if (file != null && ".editorconfig".equals(file.getName())) changedConfigs.add(file);
        }
        if (renamed.isEmpty() && changedConfigs.isEmpty()) return;
        ApplicationManager.getApplication().invokeLater(() -> {
            FileDocumentManager documents = FileDocumentManager.getInstance();
            for (VirtualFile file : renamed) {
                var document = documents.getCachedDocument(file);
                if (document == null) continue;
                for (Editor editor : EditorFactory.getInstance().getEditors(document)) sync(editor);
            }
            for (Editor editor : touchedSnapshot()) {
                if (editor.isDisposed()) continue;
                VirtualFile file = fileOf(editor);
                if (file == null) continue;
                for (VirtualFile config : changedConfigs) {
                    if (under(file, config.getParent())) {
                        sync(editor);
                        break;
                    }
                }
            }
        });
    }

    @Override public void codeStyleSettingsChanged(@NotNull CodeStyleSettingsChangeEvent event) {
        Project project = event.getProject();
        ApplicationManager.getApplication().invokeLater(() -> {
            if (project.isDisposed()) return;
            for (Editor editor : touchedSnapshot()) {
                if (!editor.isDisposed() && editor.getProject() == project) sync(editor);
            }
        });
    }

    private static void sync(Editor editor) {
        if (editor.isDisposed()) return;
        Project project = editor.getProject();
        VirtualFile file = fileOf(editor);
        if (project == null || project.isDisposed() || file == null || !file.isValid()) return;
        AtomicInteger counter = editor.getUserData(GENERATION);
        if (counter == null) {
            counter = new AtomicInteger();
            editor.putUserData(GENERATION, counter);
        }
        int generation = counter.incrementAndGet();
        if (isNagi(file)) {
            TOUCHED.add(editor);
            editor.getSettings().setUseTabCharacter(false);
        } else if (TOUCHED.contains(editor)) {
            AtomicInteger current = counter;
            ReadAction.nonBlocking(() -> CodeStyle.getIndentOptions(project, file).USE_TAB_CHARACTER)
                    .expireWith(project)
                    .coalesceBy(GENERATION, editor)
                    .finishOnUiThread(ModalityState.any(), useTabs -> {
                        if (editor.isDisposed() || current.get() != generation || fileOf(editor) != file
                                || isNagi(file)) return;
                        editor.getSettings().setUseTabCharacter(useTabs);
                    })
                    .submit(AppExecutorUtil.getAppExecutorService());
        }
    }

    private static boolean isNagi(VirtualFile file) {
        return file.getFileType() == NagiFileType.HIGH || file.getFileType() == NagiFileType.LOW;
    }

    private static VirtualFile fileOf(Editor editor) {
        VirtualFile file = editor.getVirtualFile();
        return file != null ? file : FileDocumentManager.getInstance().getFile(editor.getDocument());
    }

    private static boolean under(VirtualFile file, VirtualFile directory) {
        for (VirtualFile parent = file.getParent(); parent != null; parent = parent.getParent()) {
            if (parent == directory) return true;
        }
        return false;
    }

    private static Editor[] touchedSnapshot() {
        synchronized (TOUCHED) { return TOUCHED.toArray(Editor[]::new); }
    }
}
