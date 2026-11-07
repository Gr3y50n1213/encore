package uk.gcartee.encore.app.ui.theme

import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import uk.gcartee.encore.app.R
import uk.gcartee.encore.tokens.EncoreTokens.Breakpoints
import uk.gcartee.encore.tokens.EncoreTokens.Typography.Scale.Compact as C
import uk.gcartee.encore.tokens.EncoreTokens.Typography.Scale.Wide as W

/** Inter, bundled (plan section 3), one variable file for every weight. */
private val weights = listOf(400, 500, 600, 700)

@OptIn(androidx.compose.ui.text.ExperimentalTextApi::class)
val Inter = FontFamily(
    weights.map { w ->
        Font(
            R.font.inter_variable,
            weight = FontWeight(w),
            variationSettings = FontVariation.Settings(FontVariation.weight(w)),
        )
    },
)

private fun role(size: Double, weight: Int, lineHeight: Double) = TextStyle(
    fontFamily = Inter,
    fontSize = size.sp,
    fontWeight = FontWeight(weight),
    lineHeight = lineHeight.sp,
)

/**
 * The type roles of design/tokens.json `typography.scale`: compact under 600 dp of
 * window width, wide from 600, never by device. Roles the wide scale does not set use
 * the compact one.
 */
data class EncoreType(
    val displayTitle: TextStyle,
    val onboardingTitle: TextStyle,
    val pageTitle: TextStyle,
    val collectionTitle: TextStyle,
    val sheetTitle: TextStyle,
    val sectionHeader: TextStyle,
    val nowPlayingTitle: TextStyle,
    val nowPlayingSubtitle: TextStyle,
    val lyricLine: TextStyle,
    val tileTitle: TextStyle,
    val tileSubtitle: TextStyle,
    val songRowTitle: TextStyle,
    val listRowTitle: TextStyle,
    val rowValue: TextStyle,
    val groupLabel: TextStyle,
    val navLabel: TextStyle,
    val navLabelSelected: TextStyle,
    val buttonLabel: TextStyle,
    val buttonLabelSelected: TextStyle,
    val body: TextStyle,
    val caption: TextStyle,
) {
    companion object {
        val compact = EncoreType(
            displayTitle = with(C.DisplayTitle) { role(size, weight, lineHeight) },
            onboardingTitle = with(C.OnboardingTitle) { role(size, weight, lineHeight) },
            pageTitle = with(C.PageTitle) { role(size, weight, lineHeight) },
            collectionTitle = with(C.CollectionTitle) { role(size, weight, lineHeight) },
            sheetTitle = with(C.SheetTitle) { role(size, weight, lineHeight) },
            sectionHeader = with(C.SectionHeader) { role(size, weight, lineHeight) },
            nowPlayingTitle = with(C.NowPlayingTitle) { role(size, weight, lineHeight) },
            nowPlayingSubtitle = with(C.NowPlayingSubtitle) { role(size, weight, lineHeight) },
            lyricLine = with(C.LyricLine) { role(size, weight, lineHeight) },
            tileTitle = with(C.TileTitle) { role(size, weight, lineHeight) },
            tileSubtitle = with(C.TileSubtitle) { role(size, weight, lineHeight) },
            songRowTitle = with(C.SongRowTitle) { role(size, weight, lineHeight) },
            listRowTitle = with(C.ListRowTitle) { role(size, weight, lineHeight) },
            rowValue = with(C.RowValue) { role(size, weight, lineHeight) },
            groupLabel = with(C.GroupLabel) { role(size, weight, lineHeight) },
            navLabel = with(C.NavLabel) { role(size, weight, lineHeight) },
            navLabelSelected = with(C.NavLabel) { role(size, selectedWeight, lineHeight) },
            buttonLabel = with(C.ButtonLabel) { role(size, weight, lineHeight) },
            buttonLabelSelected = with(C.ButtonLabel) { role(size, selectedWeight, lineHeight) },
            body = with(C.Body) { role(size, weight, lineHeight) },
            caption = with(C.Caption) { role(size, weight, lineHeight) },
        )

        val wide = compact.copy(
            pageTitle = with(W.PageTitle) { role(size, weight, lineHeight) },
            collectionTitle = with(W.CollectionTitle) { role(size, weight, lineHeight) },
            sectionHeader = with(W.SectionHeader) { role(size, weight, lineHeight) },
            nowPlayingTitle = with(W.NowPlayingTitle) { role(size, weight, lineHeight) },
            nowPlayingSubtitle = with(W.NowPlayingSubtitle) { role(size, weight, lineHeight) },
            lyricLine = with(W.LyricLine) { role(size, weight, lineHeight) },
            tileTitle = with(W.TileTitle) { role(size, weight, lineHeight) },
            tileSubtitle = with(W.TileSubtitle) { role(size, weight, lineHeight) },
            songRowTitle = with(W.SongRowTitle) { role(size, weight, lineHeight) },
            groupLabel = with(W.GroupLabel) { role(size, weight, lineHeight) },
            body = with(W.Body) { role(size, weight, lineHeight) },
            caption = with(W.Caption) { role(size, weight, lineHeight) },
        )

        fun forWidth(widthDp: Int): EncoreType = if (widthDp >= Breakpoints.Medium.min) wide else compact
    }
}

val LocalEncoreType = staticCompositionLocalOf { EncoreType.compact }

/** The type scale for the current window: `EncoreTheme.type.pageTitle` and so on. */
object EncoreThemeType {
    val current: EncoreType
        @Composable @ReadOnlyComposable get() = LocalEncoreType.current
}

@Composable
@ReadOnlyComposable
fun typeForWindow(): EncoreType = EncoreType.forWidth(LocalConfiguration.current.screenWidthDp)
