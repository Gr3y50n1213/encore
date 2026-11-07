package uk.gcartee.encore.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import uk.gcartee.encore.app.ui.HelloScreen
import uk.gcartee.encore.app.ui.theme.EncoreTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Edge to edge cannot be turned off at API 36; every screen reads its insets.
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent {
            EncoreTheme {
                HelloScreen()
            }
        }
    }
}
