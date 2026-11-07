package uk.gcartee.encore.app

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uk.gcartee.encore.core.appName
import uk.gcartee.encore.core.coreVersion
import uk.gcartee.encore.core.hello

/**
 * Phase 1's "done when": the app's first screen shows text that comes from the Rust core,
 * through the generated Kotlin bindings, JNA and libencore_core.so on a real Android runtime.
 */
@RunWith(AndroidJUnit4::class)
class HelloScreenTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    @Test
    fun theScreenShowsTheCoresWords() {
        val name = appName()
        val greeting = hello("Android")
        // The core's own sentence (core/encore-core/src/lib.rs), not one typed in the app.
        assertEquals("Hello, Android, from the $name core.", greeting)
        assertTrue(coreVersion().matches(Regex("""\d+\.\d+\.\d+.*""")))

        rule.onNodeWithTag("title").assertIsDisplayed().assertTextEquals(name)
        rule.onNodeWithTag("greeting").assertIsDisplayed().assertTextEquals(greeting)
        rule.onNodeWithTag("version").assertIsDisplayed().assertTextEquals("Core ${coreVersion()}")
    }

    @Test
    fun theLauncherNameIsTheCoresName() {
        assertEquals(appName(), rule.activity.getString(R.string.app_name))
    }
}
