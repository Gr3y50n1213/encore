package uk.gcartee.encore.app.playback.tv

import org.junit.Assert.assertEquals
import org.junit.Test

/** Roku's ECP replies, written as Roku's ECP guide shows them (plan T5). */
class RokuTest {
    @Test
    fun the_media_player_report_gives_state_and_times() {
        val playing = """<?xml version="1.0" encoding="UTF-8" ?>
            <player error="false" state="play">
                <plugin bandwidth="18432 bps" id="dev" name="Encore"/>
                <format audio="mp3" captions="none" container="mp3" drm="none" video="none"/>
                <buffering current="1000" max="1000" target="0"/>
                <position>83210 ms</position>
                <duration>201000 ms</duration>
                <is_live>false</is_live>
            </player>"""
        assertEquals(Triple(RendererState.PLAYING, 83_210L, 201_000L), Roku.parseMediaPlayer(playing))
        assertEquals(RendererState.PAUSED, Roku.parseMediaPlayer("""<player error="false" state="pause"><position>5 ms</position></player>""").first)
        assertEquals(RendererState.TRANSITIONING, Roku.parseMediaPlayer("""<player state="buffer"/>""").first)
        // Nothing open: no media, no times.
        assertEquals(Triple(RendererState.NO_MEDIA, 0L, 0L), Roku.parseMediaPlayer("""<player error="false" state="none"/>"""))
    }

    @Test
    fun the_channel_is_given_plain_encoded_parameters() {
        assertEquals(
            "url=http%3A%2F%2F192.168.1.5%3A41234%2Fab%2Fsong%2F0&title=Slow%20Burn%20%26%20Co",
            Roku.query(linkedMapOf("url" to "http://192.168.1.5:41234/ab/song/0", "title" to "Slow Burn & Co")),
        )
    }
}
