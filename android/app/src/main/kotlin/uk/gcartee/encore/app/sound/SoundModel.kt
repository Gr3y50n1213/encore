package uk.gcartee.encore.app.sound

import android.content.Context
import java.io.File
import uk.gcartee.encore.core.EncoreCore

/**
 * The sound model (Phase 10; docs/adr/core-0019): `yamnet-v1.bin` ships inside the app as an
 * asset (from core/encore-analysis/model, Apache-2.0, with its NOTICE beside it) and the core
 * reads it by path. It is copied out once per app version into no-backup storage, then handed
 * to the core. Call off the main thread; safe to call often.
 */
object SoundModel {
    private const val ASSET = "sound/yamnet-v1.bin"
    private const val FILE = "yamnet-v1.bin"

    @Volatile private var loaded = false

    @Synchronized
    fun ensure(context: Context, core: EncoreCore): Boolean {
        if (loaded) return true
        return runCatching {
            val dir = File(context.noBackupFilesDir, "sound").apply { mkdirs() }
            val file = File(dir, FILE)
            val stamp = File(dir, "$FILE.version")
            val version = context.packageManager.getPackageInfo(context.packageName, 0).lastUpdateTime.toString()
            if (!file.exists() || !stamp.exists() || stamp.readText() != version) {
                // Written beside it and moved into place, so a copy cut short is never read.
                val part = File(dir, "$FILE.part")
                context.assets.open(ASSET).use { input -> part.outputStream().use { input.copyTo(it) } }
                file.delete()
                check(part.renameTo(file)) { "couldn't put the sound model in place" }
                stamp.writeText(version)
            }
            core.setSoundModel(file.absolutePath)
            loaded = true
        }.isSuccess
    }
}
