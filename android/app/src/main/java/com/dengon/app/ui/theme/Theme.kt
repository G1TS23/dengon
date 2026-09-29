package com.dengon.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// Palette Material 3 : bleu profond (confiance, sobriété). Les paires
// texte/fond respectent le contraste WCAG AA (≥ 4,5:1 pour le texte courant) :
// `onPrimary` blanc sur 0B57D0 ≈ 6,6:1, `onSurface` sur `surface` ≈ 15:1, etc.
private val ClairPrimaire = Color(0xFF0B57D0)
private val ClairSurPrimaire = Color(0xFFFFFFFF)
private val ClairConteneur = Color(0xFFD3E3FD)
private val ClairSurConteneur = Color(0xFF041E49)
private val ClairSecondaire = Color(0xFF00639B)
private val ClairFond = Color(0xFFFDFBFF)
private val ClairSurface = Color(0xFFFDFBFF)
private val ClairSurfaceVariante = Color(0xFFE1E2EC)
private val ClairSurVariante = Color(0xFF44474F)
private val ClairErreur = Color(0xFFB3261E)
private val ClairConteneurErreur = Color(0xFFF9DEDC)
private val ClairSurConteneurErreur = Color(0xFF410E0B)
private val ClairContour = Color(0xFF6F7280)

private val SombrePrimaire = Color(0xFFA8C7FA)
private val SombreSurPrimaire = Color(0xFF062E6F)
private val SombreConteneur = Color(0xFF0842A0)
private val SombreSurConteneur = Color(0xFFD3E3FD)
private val SombreSecondaire = Color(0xFF7FCFFF)
private val SombreFond = Color(0xFF111318)
private val SombreSurface = Color(0xFF111318)
private val SombreSurfaceVariante = Color(0xFF44474F)
private val SombreSurVariante = Color(0xFFC4C6D0)
private val SombreErreur = Color(0xFFF2B8B5)
private val SombreConteneurErreur = Color(0xFF8C1D18)
private val SombreSurConteneurErreur = Color(0xFFF9DEDC)
private val SombreContour = Color(0xFF8E9099)

private val SchemaClair = lightColorScheme(
    primary = ClairPrimaire,
    onPrimary = ClairSurPrimaire,
    primaryContainer = ClairConteneur,
    onPrimaryContainer = ClairSurConteneur,
    secondary = ClairSecondaire,
    background = ClairFond,
    surface = ClairSurface,
    onSurface = Color(0xFF1B1B1F),
    surfaceVariant = ClairSurfaceVariante,
    onSurfaceVariant = ClairSurVariante,
    error = ClairErreur,
    errorContainer = ClairConteneurErreur,
    onErrorContainer = ClairSurConteneurErreur,
    outline = ClairContour,
)

private val SchemaSombre = darkColorScheme(
    primary = SombrePrimaire,
    onPrimary = SombreSurPrimaire,
    primaryContainer = SombreConteneur,
    onPrimaryContainer = SombreSurConteneur,
    secondary = SombreSecondaire,
    background = SombreFond,
    surface = SombreSurface,
    onSurface = Color(0xFFE3E2E6),
    surfaceVariant = SombreSurfaceVariante,
    onSurfaceVariant = SombreSurVariante,
    error = SombreErreur,
    errorContainer = SombreConteneurErreur,
    onErrorContainer = SombreSurConteneurErreur,
    outline = SombreContour,
)

// Typographie : tailles en `sp` (elles suivent la taille de police système,
// jusqu'à 200 %), interlignes généreux pour la lisibilité.
private val Typographie = Typography(
    headlineSmall = TextStyle(fontSize = 24.sp, lineHeight = 32.sp, fontWeight = FontWeight.SemiBold),
    titleLarge = TextStyle(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.SemiBold),
    titleMedium = TextStyle(fontSize = 16.sp, lineHeight = 24.sp, fontWeight = FontWeight.Medium),
    titleSmall = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    bodyLarge = TextStyle(fontSize = 16.sp, lineHeight = 24.sp),
    bodyMedium = TextStyle(fontSize = 14.sp, lineHeight = 20.sp),
    bodySmall = TextStyle(fontSize = 13.sp, lineHeight = 18.sp),
    labelLarge = TextStyle(fontSize = 14.sp, lineHeight = 20.sp, fontWeight = FontWeight.Medium),
    // 12 sp minimum : en dessous, le texte devient illisible sans zoom.
    labelSmall = TextStyle(fontSize = 12.sp, lineHeight = 16.sp, fontWeight = FontWeight.Medium),
)

private val Formes = Shapes(
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(24.dp),
)

/** Cible tactile minimale recommandée (Material / WCAG 2.5.8). */
val CibleTactileMin = 48.dp

/** Thème de l'app : Material 3, clair ou sombre selon le réglage du téléphone. */
@Composable
fun DengonTheme(sombre: Boolean = isSystemInDarkTheme(), contenu: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (sombre) SchemaSombre else SchemaClair,
        typography = Typographie,
        shapes = Formes,
        content = contenu,
    )
}
