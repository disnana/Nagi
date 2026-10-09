package com.disnana.nagi;

import com.intellij.codeInsight.completion.CompletionContributor;
import com.intellij.codeInsight.completion.CompletionParameters;
import com.intellij.codeInsight.completion.CompletionResultSet;
import com.intellij.codeInsight.lookup.LookupElementBuilder;
import org.jetbrains.annotations.NotNull;

/** Presents only compiler-verified read candidates for the current editor snapshot. */
public final class NagiCompletionContributor extends CompletionContributor {
    @Override public void fillCompletionVariants(@NotNull CompletionParameters parameters,
                                                @NotNull CompletionResultSet result) {
        var file = parameters.getOriginalFile();
        if (!NagiCompilerAction.isSourceFile(file.getVirtualFile())) return;
        var service = file.getProject().getService(NagiAssistService.class);
        var snapshot = service.fresh(file, parameters.getEditor());
        if (snapshot == null) {
            service.requestCompletion(file, parameters.getEditor());
            return;
        }
        for (var candidate : snapshot.response().completions()) {
            var item = LookupElementBuilder.create(candidate.name());
            String detail = candidate.type() != null ? candidate.type() : candidate.signature();
            if (detail != null) item = item.withTypeText(detail, true);
            if (candidate.borrowed()) item = item.withTailText(" (borrowed read)", true);
            // Inserting a name does not authorize consumption, assignment or a
            // call. The ordinary compiler validates the resulting program.
            result.addElement(item);
        }
    }
}
