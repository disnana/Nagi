package com.disnana.nagi;

import com.intellij.openapi.options.Configurable;
import java.awt.GridBagConstraints;
import java.awt.GridBagLayout;
import java.awt.Insets;
import javax.swing.JComponent;
import javax.swing.JLabel;
import javax.swing.JPanel;
import javax.swing.JSpinner;
import javax.swing.JTextField;
import javax.swing.SpinnerNumberModel;

public final class NagiConfigurable implements Configurable {
    private JTextField compilerPath;
    private JSpinner timeout;
    @Override public String getDisplayName() { return "Nagi"; }
    @Override public JComponent createComponent() {
        compilerPath = new JTextField(35);
        timeout = new JSpinner(new SpinnerNumberModel(30, 1, 300, 1));
        var panel = new JPanel(new GridBagLayout());
        var constraints = new GridBagConstraints();
        constraints.anchor = GridBagConstraints.NORTHWEST;
        constraints.insets = new Insets(4, 4, 4, 4);
        constraints.gridx = 0; constraints.gridy = 0;
        panel.add(new JLabel("Compiler executable:"), constraints);
        constraints.gridx = 1; constraints.weightx = 1; constraints.fill = GridBagConstraints.HORIZONTAL;
        panel.add(compilerPath, constraints);
        constraints.gridx = 1; constraints.gridy++;
        panel.add(new JLabel("Leave empty to use nagic from PATH. Relative paths start at the IDE project root."), constraints);
        constraints.gridx = 0; constraints.gridy++; constraints.weightx = 0;
        panel.add(new JLabel("Check timeout (seconds):"), constraints);
        constraints.gridx = 1;
        panel.add(timeout, constraints);
        constraints.gridy++; constraints.weighty = 1;
        panel.add(new JLabel("Check and Run save open files. The compiler runs only when you choose an action."), constraints);
        reset();
        return panel;
    }
    @Override public boolean isModified() {
        var settings = NagiSettings.getInstance().getState();
        return !compilerPath.getText().equals(settings.compilerPath) || ((Number)timeout.getValue()).intValue() != settings.checkTimeoutSeconds;
    }
    @Override public void apply() {
        var values = NagiSettings.getInstance().getState();
        values.compilerPath = compilerPath.getText();
        values.checkTimeoutSeconds = ((Number)timeout.getValue()).intValue();
    }
    @Override public void reset() {
        var values = NagiSettings.getInstance().getState();
        compilerPath.setText(values.compilerPath);
        timeout.setValue(values.checkTimeoutSeconds);
    }
    @Override public void disposeUIResources() { compilerPath = null; timeout = null; }
}
