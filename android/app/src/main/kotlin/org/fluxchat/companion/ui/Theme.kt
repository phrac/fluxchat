package org.fluxchat.companion.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import org.fluxchat.companion.agents.RunState

/** Brand colours (the fluxchat mark: blue tile, green prompt), for devices without dynamic colour. */
private val BrandDark = darkColorScheme(
    primary = Color(0xFFAFC6FF),
    onPrimary = Color(0xFF0B2F6B),
    primaryContainer = Color(0xFF244A91),
    onPrimaryContainer = Color(0xFFD9E2FF),
    secondary = Color(0xFF9BE09B),
    onSecondary = Color(0xFF00390F),
    secondaryContainer = Color(0xFF1E5226),
    onSecondaryContainer = Color(0xFFB6F9B4),
    tertiary = Color(0xFFDDBCE0),
    background = Color(0xFF111318),
    onBackground = Color(0xFFE2E2E9),
    surface = Color(0xFF111318),
    onSurface = Color(0xFFE2E2E9),
    surfaceVariant = Color(0xFF44464F),
    onSurfaceVariant = Color(0xFFC5C6D0),
    surfaceContainerLowest = Color(0xFF0C0E13),
    surfaceContainerLow = Color(0xFF191C20),
    surfaceContainer = Color(0xFF1D2024),
    surfaceContainerHigh = Color(0xFF282A2F),
    surfaceContainerHighest = Color(0xFF33353A),
    outline = Color(0xFF8F9099),
    outlineVariant = Color(0xFF44464F),
)

private val BrandLight = lightColorScheme(
    primary = Color(0xFF2F62C8),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFD9E2FF),
    onPrimaryContainer = Color(0xFF001A43),
    secondary = Color(0xFF2E6A33),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFB6F9B4),
    onSecondaryContainer = Color(0xFF002106),
    background = Color(0xFFF8F9FF),
    surface = Color(0xFFF8F9FF),
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color(0xFFF2F3FA),
    surfaceContainer = Color(0xFFECEDF4),
    surfaceContainerHigh = Color(0xFFE6E8EE),
    surfaceContainerHighest = Color(0xFFE1E2E9),
)

/** Run state colours: what the desktop uses (blue / amber / green / red), toned for light and dark. */
@Immutable
data class StatusColors(val working: Color, val needsYou: Color, val done: Color, val failed: Color, val needsYouContainer: Color)

private val StatusDark = StatusColors(Color(0xFF8AB4FF), Color(0xFFFFC94D), Color(0xFF6DD58C), Color(0xFFFF8A80), Color(0xFF3D3214))
private val StatusLight = StatusColors(Color(0xFF2F62C8), Color(0xFF9A6700), Color(0xFF1E7B3A), Color(0xFFBA1A1A), Color(0xFFFFF0C7))

val LocalStatus = staticCompositionLocalOf { StatusDark }
private val LocalDark = staticCompositionLocalOf { true }

@Composable
@ReadOnlyComposable
fun RunState.color(): Color {
    val s = LocalStatus.current
    return when (this) {
        RunState.Working -> s.working
        RunState.NeedsYou -> s.needsYou
        RunState.Done -> s.done
        RunState.Failed -> s.failed
    }
}

private val nicksDark = listOf(0xFF8AB4FF, 0xFF6DD58C, 0xFFFF8BC6, 0xFFFFC94D, 0xFFC5A4FF, 0xFFFF8A80, 0xFF5FD7C9, 0xFFFFAB70)
private val nicksLight = listOf(0xFF2F62C8, 0xFF1E7B3A, 0xFFB0247A, 0xFF8A5A00, 0xFF6B3FC0, 0xFFB3261E, 0xFF00796B, 0xFFB25000)

/** A user's colour (name, avatar), stable per user ID. */
@Composable
@ReadOnlyComposable
fun nickColor(userId: String): Color {
    val palette = if (LocalDark.current) nicksDark else nicksLight
    return Color(palette[Math.floorMod(userId.hashCode(), palette.size)])
}

@Composable
fun FluxTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val context = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= 31 -> if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
        dark -> BrandDark
        else -> BrandLight
    }
    CompositionLocalProvider(LocalStatus provides if (dark) StatusDark else StatusLight, LocalDark provides dark) {
        MaterialTheme(
            colorScheme = scheme,
            shapes = Shapes(
                extraSmall = RoundedCornerShape(6.dp),
                small = RoundedCornerShape(10.dp),
                medium = RoundedCornerShape(16.dp),
                large = RoundedCornerShape(24.dp),
                extraLarge = RoundedCornerShape(32.dp),
            ),
            content = content,
        )
    }
}
