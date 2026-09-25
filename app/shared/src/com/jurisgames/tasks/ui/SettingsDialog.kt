package com.jurisgames.tasks.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.Settings

private val helpLabels = mapOf(
    "wifi_and_charging" to "On Wi-Fi while charging (and on Wi-Fi while open)",
    "wifi_while_open" to "On Wi-Fi while the app is open",
    "on" to "On",
    "off" to "Off",
)

/** The settings (DESIGN settings table). Settings this device doesn't have aren't shown. */
@Composable
fun SettingsDialog(node: KtNode, onSaved: (Settings) -> Unit, onClose: () -> Unit) {
    var s by remember(node) { mutableStateOf(node.settings()) }
    var error by remember { mutableStateOf("") }
    val version = remember(node) { node.diagnostics().version }
    var taps by remember { mutableStateOf(0) }
    var diagnostics by remember { mutableStateOf(false) }
    if (diagnostics) DiagnosticsDialog(node, onClose = { diagnostics = false })
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = {
            TextButton(onClick = {
                try {
                    node.setSettings(s)
                    onSaved(s)
                    onClose()
                } catch (e: Exception) {
                    error = e.message ?: "Couldn't save"
                }
            }, modifier = Modifier.testTag("settingsSave")) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
        title = { Text("Settings") },
        text = {
            Column {
                SwitchRow("Sync in the background", s.syncInBackground, "set-background") { s = s.copy(syncInBackground = it) }
                SwitchRow("Notify me when others change a list", s.notifyChanges, "set-notify") { s = s.copy(notifyChanges = it) }
                Text("Help others sync", style = MaterialTheme.typography.titleSmall)
                for (opt in s.helpOptions) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                        RadioButton(s.helpOthers == opt, { s = s.copy(helpOthers = opt) }, modifier = Modifier.testTag("help-$opt"))
                        Text(helpLabels[opt] ?: opt)
                    }
                }
                s.relayForOthers?.let { v -> SwitchRow("Relay connections for others", v, "set-relay") { s = s.copy(relayForOthers = it) } }
                s.startWithComputer?.let { v -> SwitchRow("Start with computer", v, "set-autostart") { s = s.copy(startWithComputer = it) } }
                Text("Appearance", style = MaterialTheme.typography.titleSmall)
                // Material You: offered where the platform has it (Android 12+).
                val dynamic = s.dynamicColor
                if (dynamic != null && LocalDynamicAvailable.current) SwitchRow("Use wallpaper colors", dynamic, "set-dynamic") { s = s.copy(dynamicColor = it) }
                if (dynamic != true || !LocalDynamicAvailable.current) {
                    for (t in com.jurisgames.tasks.core.themes()) {
                        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                            RadioButton(s.theme == t.id, { s = s.copy(theme = t.id) }, modifier = Modifier.testTag("theme-" + t.id))
                            Text(t.name)
                        }
                    }
                }
                if (error.isNotEmpty()) Text(error, modifier = Modifier.testTag("settingsError"))
                // Hidden diagnostics (DESIGN §2): tap the version seven times.
                Text(
                    "Version $version",
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.testTag("version").clickable { if (++taps >= 7) { taps = 0; diagnostics = true } },
                )
            }
        },
    )
}

@Composable
private fun SwitchRow(label: String, value: Boolean, tag: String, onChange: (Boolean) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
        Text(label, modifier = Modifier.weight(1f))
        Switch(value, onChange, modifier = Modifier.testTag(tag))
    }
}
