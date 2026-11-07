package uk.gcartee.encore.app

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
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
 * The core on a real Android runtime: the generated bindings, JNA and libencore_core.so.
 * Phase 1's line ("a screen whose text comes from the Rust core") now holds on Welcome,
 * whose name is the core's `app_name()`.
 */
@RunWith(AndroidJUnit4::class)
class CoreOnDeviceTest {
    @get:Rule
    val rule = createAndroidComposeRule<MainActivity>()

    @Test
    fun theCoreAnswersThroughTheBindings() {
        // The core's own sentence (core/encore-core/src/lib.rs), not one typed in the app.
        assertEquals("Hello, Android, from the ${appName()} core.", hello("Android"))
        assertTrue(coreVersion().matches(Regex("""\d+\.\d+\.\d+.*""")))
    }

    @Test
    fun theFirstScreenShowsTheCoresName() {
        // A fresh install has no profile, so the first screen is Welcome (board 22).
        rule.onNodeWithText(appName()).assertIsDisplayed()
        rule.onNodeWithText("Get started").assertIsDisplayed()
    }

    @Test
    fun theLauncherNameIsTheCoresName() {
        assertEquals(appName(), rule.activity.getString(R.string.app_name))
    }
}
