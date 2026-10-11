package com.disnana.nagi;

import com.intellij.openapi.application.ApplicationInfo;
import com.intellij.openapi.progress.ProcessCanceledException;
import com.intellij.openapi.vfs.newvfs.ManagingFS;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

/** Waits for VFS writes before an external compiler reads the saved input paths. */
final class NagiSaveBarrier {
    private NagiSaveBarrier() {}

    static void awaitDiskWrites() throws IOException {
        try {
            int baseline = ApplicationInfo.getInstance().getBuild().getBaselineVersion();
            flush(ManagingFS.class, ManagingFS.getInstance(), baseline);
        } catch (ProcessCanceledException canceled) {
            throw canceled;
        } catch (RuntimeException | LinkageError failure) {
            throw new IOException("The IDE save-to-disk service is unavailable.", failure);
        }
    }

    static void flush(Class<?> api, Object service, int baseline) throws IOException {
        flush(api, service, baseline, (method, target) -> method.invoke(target));
    }

    @FunctionalInterface
    interface Invoker {
        void invoke(Method method, Object target) throws InvocationTargetException, IllegalAccessException;
    }

    static void flush(Class<?> api, Object service, int baseline, Invoker invoker) throws IOException {
        Method flush;
        try {
            // Public but Experimental since 262. Reflection preserves the 251 build target.
            flush = api.getMethod("flushPendingUpdates");
        } catch (NoSuchMethodException absent) {
            // These pinned baselines save document bytes synchronously and have no flush API.
            if (baseline == 251 || baseline == 252 || baseline == 253 || baseline == 261) return;
            throw new IOException("This IDE has no supported save-to-disk barrier (build " + baseline + ").", absent);
        } catch (SecurityException | LinkageError failure) {
            throw new IOException("Cannot inspect the IDE save-to-disk barrier.", failure);
        }

        if (flush.getReturnType() != void.class || Modifier.isStatic(flush.getModifiers())) {
            throw new IOException("The IDE save-to-disk barrier has an unexpected signature.");
        }
        try {
            invoker.invoke(flush, service);
        } catch (InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof ProcessCanceledException canceled) throw canceled;
            if (cause instanceof IOException io) throw io;
            throw new IOException("The IDE could not flush pending file writes.", cause == null ? failure : cause);
        } catch (IllegalAccessException | IllegalArgumentException | SecurityException | LinkageError failure) {
            throw new IOException("Cannot invoke the IDE save-to-disk barrier.", failure);
        }
    }
}
