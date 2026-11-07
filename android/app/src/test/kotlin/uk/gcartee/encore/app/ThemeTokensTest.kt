package uk.gcartee.encore.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uk.gcartee.encore.app.ui.theme.irisDarkScheme
import uk.gcartee.encore.app.ui.theme.irisLightScheme
import uk.gcartee.encore.app.ui.theme.tokenColor
import uk.gcartee.encore.tokens.EncoreTokens.Color.Accents.Iris

class ThemeTokensTest {
    @Test
    fun hexTokensBecomeOpaqueColours() {
        val c = tokenColor("#6146CB")
        assertEquals(1f, c.alpha)
        assertEquals(0x61 / 255f, c.red, 0.002f)
        assertEquals(0x46 / 255f, c.green, 0.002f)
        assertEquals(0xCB / 255f, c.blue, 0.002f)
    }

    @Test
    fun aMalformedTokenIsRefused() {
        assertThrows(IllegalArgumentException::class.java) { tokenColor("6146CB") }
        assertThrows(IllegalArgumentException::class.java) { tokenColor("#6146CB80") }
    }

    @Test
    fun theSchemesAreTheTokenRoles() {
        val light = irisLightScheme()
        assertEquals(tokenColor(Iris.Light.primary), light.primary)
        assertEquals(tokenColor(Iris.Light.surface), light.surface)
        assertEquals(tokenColor(Iris.Light.surfaceContainerHighest), light.surfaceContainerHighest)
        assertEquals(tokenColor(Iris.Light.scrim), light.scrim)
        val dark = irisDarkScheme()
        assertEquals(tokenColor(Iris.Dark.primary), dark.primary)
        assertEquals(tokenColor(Iris.Dark.onSurface), dark.onSurface)
        assertEquals(tokenColor(Iris.Dark.outlineVariant), dark.outlineVariant)
    }

    /** The launcher background is XML, which cannot read the constants: keep it the Iris seed. */
    @Test
    fun theLauncherBackgroundIsTheIrisSeed() {
        val xml = File("src/main/res/values/ic_launcher.xml").readText()
        val value = Regex("""name="ic_launcher_background">(#[0-9A-Fa-f]{6})<""").find(xml)!!.groupValues[1]
        assertEquals(Iris.seed.uppercase(), value.uppercase())
    }
}
