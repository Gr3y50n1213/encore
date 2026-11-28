package uk.gcartee.encore.wear

import android.os.SystemClock
import androidx.wear.protolayout.ActionBuilders
import androidx.wear.protolayout.ColorBuilders.argb
import androidx.wear.protolayout.DimensionBuilders.dp
import androidx.wear.protolayout.DimensionBuilders.sp
import androidx.wear.protolayout.LayoutElementBuilders
import androidx.wear.protolayout.ModifiersBuilders
import androidx.wear.protolayout.ResourceBuilders
import androidx.wear.protolayout.TimelineBuilders
import androidx.wear.protolayout.material3.materialScope
import androidx.wear.protolayout.material3.primaryLayout
import androidx.wear.protolayout.material3.text
import androidx.wear.protolayout.types.layoutString
import androidx.wear.tiles.RequestBuilders
import androidx.wear.tiles.TileBuilders
import androidx.wear.tiles.TileService
import com.google.common.util.concurrent.ListenableFuture
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import uk.gcartee.encore.wearlink.TileChip
import uk.gcartee.encore.wearlink.WatchPrivacy
import uk.gcartee.encore.wearlink.WearCommand

/**
 * Mock-up 50, screen 5: the Mixes tile. Up to four chips, the pinned mixes then Liked songs,
 * and "Tap a mix to play". A tap sends `mix:<id>` (or `liked`) to the phone, which starts
 * it the way its own Quick Settings tile does. On a locked watch it shows no mix names.
 */
class MixesTileService : TileService() {

    override fun onTileRequest(requestParams: RequestBuilders.TileRequest): ListenableFuture<TileBuilders.Tile> {
        clicked(requestParams.currentState.lastClickableId)
        WatchStore.load(this)
        val locked = watchLocked(this)
        val chips = WatchStore.mixes.value.chips()
        val name = getString(R.string.app_name)
        val layout = materialScope(this, requestParams.deviceConfiguration) {
            primaryLayout(
                titleSlot = { text(name.layoutString) },
                mainSlot = {
                    when {
                        !WatchPrivacy.showMixes(locked) -> message(getString(R.string.tile_locked))
                        chips.isEmpty() -> message(getString(R.string.tile_empty, name))
                        else -> grid(chips)
                    }
                },
            )
        }
        val tile = TileBuilders.Tile.Builder()
            .setResourcesVersion(RESOURCES_VERSION)
            .setTileTimeline(TimelineBuilders.Timeline.fromLayoutElement(layout))
            .build()
        return Done(tile)
    }

    override fun onTileResourcesRequest(requestParams: RequestBuilders.ResourcesRequest): ListenableFuture<ResourceBuilders.Resources> =
        Done(ResourceBuilders.Resources.Builder().setVersion(RESOURCES_VERSION).build())

    /** A chip's tap comes back as a tile request with its ID; each tap is sent once. */
    private fun clicked(id: String) {
        if (id.isEmpty()) return
        val at = SystemClock.elapsedRealtime()
        if (id == lastClick && at - lastClickAt < SAME_TAP_MS) return
        lastClick = id
        lastClickAt = at
        val command = WearCommand.parse(id)
        if (command is WearCommand.PlayMix || command == WearCommand.PlayLiked) PhoneLink.send(this, command)
    }

    private fun message(words: String): LayoutElementBuilders.LayoutElement =
        LayoutElementBuilders.Text.Builder()
            .setText(words)
            .setMaxLines(3)
            .setMultilineAlignment(LayoutElementBuilders.TEXT_ALIGN_CENTER)
            .setFontStyle(LayoutElementBuilders.FontStyle.Builder().setSize(sp(13f)).setColor(argb(WORDS)).build())
            .build()

    private fun grid(chips: List<TileChip>): LayoutElementBuilders.LayoutElement {
        val column = LayoutElementBuilders.Column.Builder()
            .setHorizontalAlignment(LayoutElementBuilders.HORIZONTAL_ALIGN_CENTER)
        chips.chunked(2).forEachIndexed { row, pair ->
            if (row > 0) column.addContent(LayoutElementBuilders.Spacer.Builder().setHeight(dp(GAP)).build())
            val r = LayoutElementBuilders.Row.Builder()
            pair.forEachIndexed { i, chip ->
                if (i > 0) r.addContent(LayoutElementBuilders.Spacer.Builder().setWidth(dp(GAP)).build())
                r.addContent(chip(chip, row * 2 + i))
            }
            column.addContent(r.build())
        }
        column.addContent(LayoutElementBuilders.Spacer.Builder().setHeight(dp(GAP)).build())
        column.addContent(
            LayoutElementBuilders.Text.Builder()
                .setText(getString(R.string.tile_hint))
                .setMaxLines(1)
                .setFontStyle(LayoutElementBuilders.FontStyle.Builder().setSize(sp(11f)).setColor(argb(HINT)).build())
                .build(),
        )
        return column.build()
    }

    private fun chip(chip: TileChip, index: Int): LayoutElementBuilders.LayoutElement {
        val (title, colour, ink) = when (chip) {
            is TileChip.Mix -> Triple(
                chip.mix.title.ifBlank { getString(R.string.tile_label) },
                chip.mix.colour.takeIf { it != 0 } ?: FALLBACK[index % FALLBACK.size],
                chip.mix.text.takeIf { it != 0 && chip.mix.colour != 0 } ?: INK,
            )
            TileChip.Liked -> Triple(getString(R.string.tile_liked), LIKED, INK)
        }
        val label = if (title.length > MAX_LABEL) title.take(MAX_LABEL - 1).trimEnd() + "…" else title
        return LayoutElementBuilders.Box.Builder()
            .setWidth(dp(CHIP_W))
            .setHeight(dp(CHIP_H))
            .setModifiers(
                ModifiersBuilders.Modifiers.Builder()
                    .setBackground(
                        ModifiersBuilders.Background.Builder()
                            .setColor(argb(colour))
                            .setCorner(ModifiersBuilders.Corner.Builder().setRadius(dp(CHIP_RADIUS)).build())
                            .build(),
                    )
                    .setClickable(
                        ModifiersBuilders.Clickable.Builder()
                            .setId(chip.command.wire)
                            .setOnClick(ActionBuilders.LoadAction.Builder().build())
                            .build(),
                    )
                    .setSemantics(ModifiersBuilders.Semantics.Builder().setContentDescription(title).build())
                    .build(),
            )
            .addContent(
                LayoutElementBuilders.Text.Builder()
                    .setText(label)
                    .setMaxLines(1)
                    .setFontStyle(
                        LayoutElementBuilders.FontStyle.Builder()
                            .setSize(sp(11f))
                            .setWeight(LayoutElementBuilders.FONT_WEIGHT_BOLD)
                            .setColor(argb(ink))
                            .build(),
                    )
                    .build(),
            )
            .build()
    }

    /** The tile is built from what is saved already, so its future is done at once. */
    private class Done<T>(private val value: T) : ListenableFuture<T> {
        override fun addListener(listener: Runnable, executor: Executor) = executor.execute(listener)
        override fun cancel(mayInterruptIfRunning: Boolean): Boolean = false
        override fun isCancelled(): Boolean = false
        override fun isDone(): Boolean = true
        override fun get(): T = value
        override fun get(timeout: Long, unit: TimeUnit): T = value
    }

    private companion object {
        const val RESOURCES_VERSION = "1"
        const val SAME_TAP_MS = 2_000L
        const val MAX_LABEL = 10
        // Mock-up 50, screen 5: 62 by 40 there; 48 high here, the smallest touch target.
        const val CHIP_W = 64f
        const val CHIP_H = 48f
        const val CHIP_RADIUS = 14f
        const val GAP = 6f
        const val INK = 0xFF1B1530.toInt()
        const val HINT = 0xFFB9B2CF.toInt()
        const val WORDS = 0xFFF1ECFF.toInt()
        const val LIKED = 0xFFE8DDFF.toInt()
        val FALLBACK = intArrayOf(0xFFFFB59C.toInt(), 0xFFA9D7FF.toInt(), 0xFFC8F0A8.toInt(), 0xFFE8DDFF.toInt())

        @Volatile var lastClick: String? = null
        @Volatile var lastClickAt = 0L
    }
}
