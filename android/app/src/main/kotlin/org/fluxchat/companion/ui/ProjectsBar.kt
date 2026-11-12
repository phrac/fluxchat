package org.fluxchat.companion.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.RowScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.SmartToy
import androidx.compose.material3.AssistChip
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalConfiguration

/** Below this width the update chip says just "Update": beside three buttons its full label left the title "Proj…". */
private const val FULL_CHIP_MIN_WIDTH_DP = 400

/**
 * The Projects bar's actions. New project and Search are what the screen is
 * used for, so they stay buttons; history, agents and settings are in the
 * menu. As five buttons plus the update chip they were wider than a 360 dp
 * phone, and the last one — Settings — was pushed off the bar.
 */
@Composable
internal fun RowScope.ProjectsActions(update: Boolean, onUpdate: () -> Unit, onNew: () -> Unit, go: (Screen) -> Unit) {
    if (update) {
        // Subtle: a plain assist chip, no badge or colour shout.
        val roomy = LocalConfiguration.current.screenWidthDp >= FULL_CHIP_MIN_WIDTH_DP
        AssistChip(onClick = onUpdate, label = { Text(if (roomy) "Update available" else "Update", maxLines = 1) })
    }
    IconButton(onClick = onNew) { Icon(Icons.Outlined.Add, "New project") }
    IconButton(onClick = { go(Screen.Search) }) { Icon(Icons.Outlined.Search, "Search sessions") }
    var menu by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, "More") }
        // A menu is its own window, above the lock screen: not while the app is locked.
        DropdownMenu(expanded = menu && unlocked(), onDismissRequest = { menu = false }) {
            val item: @Composable (String, ImageVector, Screen) -> Unit = { label, icon, screen ->
                DropdownMenuItem(
                    text = { Text(label) },
                    leadingIcon = { Icon(icon, null) },
                    onClick = { menu = false; go(screen) },
                )
            }
            item("Run history", Icons.Outlined.History, Screen.History)
            item("Your agents", Icons.Outlined.SmartToy, Screen.Agents)
            item("Settings", Icons.Outlined.Settings, Screen.Settings)
        }
    }
}
