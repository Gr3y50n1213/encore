package uk.gcartee.encore.app

import org.junit.Assert.assertEquals
import org.junit.Test
import uk.gcartee.encore.app.ui.home.releaseTitle

class ReleaseTitleTest {
    @Test
    fun dashSingle() = assertEquals("Paper Boats" to "Single", releaseTitle("Paper Boats - Single"))

    @Test
    fun enDashEp() = assertEquals("Night Drive" to "EP", releaseTitle("Night Drive – EP"))

    @Test
    fun emDashNoSpaces() = assertEquals("Night Drive" to "EP", releaseTitle("Night Drive—EP"))

    @Test
    fun parenthesesSingle() = assertEquals("Lanterns" to "Single", releaseTitle("Lanterns (Single)"))

    @Test
    fun bracketsEpAnyCase() = assertEquals("Lanterns" to "EP", releaseTitle("Lanterns [ep]"))

    @Test
    fun plainTitleUnchanged() = assertEquals("Northern Lights" to null, releaseTitle("Northern Lights"))

    @Test
    fun tagAloneUnchanged() = assertEquals("Single" to null, releaseTitle("Single"))

    @Test
    fun wordInsideTitleUnchanged() = assertEquals("Single File Line" to null, releaseTitle("Single File Line"))

    @Test
    fun otherSuffixUnchanged() = assertEquals("Paper Boats - Deluxe" to null, releaseTitle("Paper Boats - Deluxe"))
}
