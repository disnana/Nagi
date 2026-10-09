package com.disnana.nagi;

import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer;
import com.intellij.openapi.application.ApplicationManager;
import com.intellij.openapi.application.ReadAction;
import com.intellij.openapi.util.Computable;
import com.intellij.psi.PsiFile;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

/** Small public-API compatibility boundary for the supported 251 through EAP SDKs. */
final class NagiPlatform {
    private static final Method RESTART;
    static {
        try {
            Method method;
            try { method = DaemonCodeAnalyzer.class.getMethod("restart", PsiFile.class, Object.class); }
            catch (NoSuchMethodException stable251) {
                // 251 exposes only restart(PsiFile), where it is not deprecated.
                // Newer SDKs always take the public, reason-aware replacement.
                method = DaemonCodeAnalyzer.class.getMethod("restart", PsiFile.class);
            }
            RESTART = method;
        } catch (NoSuchMethodException failure) { throw new ExceptionInInitializerError(failure); }
    }
    private NagiPlatform() {}

    static <T> T read(Computable<T> action) {
        var application = ApplicationManager.getApplication();
        if (application.isReadAccessAllowed()) return action.compute();
        if (application.isDispatchThread()) {
            // UI callbacks capture only small identities/stamps, never files or
            // compiler work. Stable public Application API on all supported SDKs.
            return application.runReadAction(action);
        }
        return ReadAction.nonBlocking(action::compute).executeSynchronously();
    }

    static void restart(PsiFile file) {
        try {
            var analyzer = DaemonCodeAnalyzer.getInstance(file.getProject());
            if (RESTART.getParameterCount() == 2) RESTART.invoke(analyzer, file, "Nagi compiler snapshot changed");
            else RESTART.invoke(analyzer, file);
        } catch (IllegalAccessException | InvocationTargetException failure) {
            throw new IllegalStateException("Could not refresh Nagi highlighting using the public IDE API", failure);
        }
    }
}
