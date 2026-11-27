package uk.gcartee.encore.app.core

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uk.gcartee.encore.core.LocalFolderInfo
import uk.gcartee.encore.core.LocalFoundFile

/** A look through a folder in progress (mock-up 49, screen 3). */
data class LocalLook(val folderId: UInt, val folderName: String, val done: Int, val total: Int)

/**
 * Music files on this phone (Phase 12 stage 4, ADR core-0024, mock-up 49). The listener
 * picks folders in Android's own folder picker; Encore keeps read access to those
 * folders only, walks them, and hands each music file to the core, which reads its tags.
 * A file is handed over as `/proc/self/fd/<n>` of a descriptor held open for the call,
 * so the core never needs a path or a permission of its own. Files are never changed.
 */
class LocalMusic(private val context: Context, private val host: CoreHost) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val one = Mutex()
    private val _look = MutableStateFlow<LocalLook?>(null)
    /** The look running now, if any. */
    val look: StateFlow<LocalLook?> = _look.asStateFlow()
    private val _changes = MutableStateFlow(0)
    /** Bumped when the folders or their songs change. */
    val changes: StateFlow<Int> = _changes.asStateFlow()
    @Volatile private var lookedThisRun = false

    fun folders(): List<LocalFolderInfo> = runCatching { host.core.localFolders() }.getOrDefault(emptyList())

    /** Whether new songs are looked for by themselves (mock-up 49 screen 2's switch, on by default). */
    fun autoLook(): Boolean = prefs().getBoolean(AUTO_KEY, true)

    fun setAutoLook(on: Boolean) = prefs().edit().putBoolean(AUTO_KEY, on).apply()

    private fun prefs() = context.getSharedPreferences("device-music-${host.core.currentProfile().orEmpty()}", Context.MODE_PRIVATE)

    /** A folder chosen in the picker: keep access to it, add it, and look through it. */
    fun addFolder(tree: Uri): LocalFolderInfo? {
        runCatching {
            context.contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }.onFailure { Log.w(TAG, "couldn't keep access to the folder", it) }
        val folder = runCatching { host.core.addLocalFolder(tree.toString(), nameOf(tree)) }.getOrElse {
            Log.w(TAG, "couldn't add the folder", it)
            return null
        }
        _changes.value++
        lookThrough(folder)
        return folder
    }

    /** Stop using a folder: its songs leave Encore, the files stay. */
    fun removeFolder(folder: LocalFolderInfo) {
        scope.launch {
            one.withLock {
                runCatching { host.core.removeLocalFolder(folder.folderId) }
                runCatching {
                    context.contentResolver.releasePersistableUriPermission(Uri.parse(folder.uri), Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
                _changes.value++
                host.sync.libraryChanged()
            }
        }
    }

    /** "Look again now", and the look when Encore opens (once per run, when it's on). */
    fun lookThroughAll(always: Boolean = true) {
        if (!always && (lookedThisRun || !autoLook())) return
        lookedThisRun = true
        folders().forEach(::lookThrough)
    }

    fun lookThrough(folder: LocalFolderInfo) {
        scope.launch {
            one.withLock { runCatching { walk(folder) }.onFailure { Log.w(TAG, "the look stopped", it) } }
            _look.value = null
        }
    }

    private suspend fun walk(folder: LocalFolderInfo) {
        val tree = Uri.parse(folder.uri)
        val files = runCatching { musicIn(tree) }.getOrElse {
            // The folder can't be reached (an SD card taken out, access taken back).
            Log.w(TAG, "the folder can't be reached", it)
            runCatching { host.core.setLocalFolderAvailable(folder.folderId, false) }
            _changes.value++
            return
        }
        runCatching { host.core.setLocalFolderAvailable(folder.folderId, true) }
        val known = runCatching { host.core.localKnownFiles(folder.folderId) }.getOrDefault(emptyList())
            .associate { it.uri to (it.size to it.modifiedMs) }
        val seen = ArrayList<String>(files.size)
        var fresh = 0
        _look.value = LocalLook(folder.folderId, folder.name, 0, files.size)
        files.forEachIndexed { i, f ->
            val uri = DocumentsContract.buildDocumentUriUsingTree(tree, f.docId)
            seen += uri.toString()
            if (known[uri.toString()] != (f.size.toULong() to f.modified)) {
                add(folder, tree, uri, f)
                fresh++
            }
            // Songs appear in Library as they're found (mock-up 49, screen 3).
            if (fresh >= REFRESH_EVERY) {
                fresh = 0
                runCatching { host.core.localRefresh() }
                host.sync.libraryChanged()
            }
            if (i % PROGRESS_EVERY == 0 || i == files.lastIndex) _look.value = LocalLook(folder.folderId, folder.name, i + 1, files.size)
        }
        runCatching { host.core.localFinishLook(folder.folderId, seen) }.onFailure { Log.w(TAG, "couldn't finish the look", it) }
        _changes.value++
        host.sync.libraryChanged()
    }

    private fun add(folder: LocalFolderInfo, tree: Uri, uri: Uri, f: Doc) {
        val resolver = context.contentResolver
        val lrc = f.lrcDocId?.let { id ->
            runCatching {
                resolver.openInputStream(DocumentsContract.buildDocumentUriUsingTree(tree, id))?.use { s ->
                    s.readBytes().take(LRC_MAX).toByteArray().toString(Charsets.UTF_8)
                }
            }.getOrNull()
        }
        val cover = f.coverDocId?.let { id -> runCatching { resolver.openFileDescriptor(DocumentsContract.buildDocumentUriUsingTree(tree, id), "r") }.getOrNull() }
        try {
            resolver.openFileDescriptor(uri, "r")?.use { pfd ->
                host.core.localAddFile(
                    LocalFoundFile(
                        folderId = folder.folderId,
                        uri = uri.toString(),
                        fileName = f.name,
                        size = f.size.toULong(),
                        modifiedMs = f.modified,
                        readPath = "/proc/self/fd/${pfd.fd}",
                        lrc = lrc,
                        folderCoverPath = cover?.let { "/proc/self/fd/${it.fd}" },
                    ),
                )
            }
        } catch (e: Exception) {
            // One unreadable file doesn't stop the look; it stays out of the library.
            Log.w(TAG, "a file couldn't be read", e)
        } finally {
            runCatching { cover?.close() }
        }
    }

    private class Doc(val docId: String, val name: String, val size: Long, val modified: Long, val lrcDocId: String?, val coverDocId: String?)

    /** Every music file under the tree, with the `.lrc` file of the same name and the folder's cover beside it. */
    private fun musicIn(tree: Uri): List<Doc> {
        val resolver = context.contentResolver
        val out = ArrayList<Doc>()
        val dirs = ArrayDeque<String>().apply { add(DocumentsContract.getTreeDocumentId(tree)) }
        val cols = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        )
        while (dirs.isNotEmpty()) {
            val dir = dirs.removeFirst()
            class Child(val id: String, val name: String, val size: Long, val modified: Long)
            val music = ArrayList<Child>()
            val lrc = HashMap<String, String>()
            var cover: String? = null
            val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, dir)
            resolver.query(children, cols, null, null, null)?.use { c ->
                while (c.moveToNext()) {
                    val id = c.getString(0) ?: continue
                    val name = c.getString(1) ?: continue
                    if (name.startsWith(".")) continue
                    when {
                        c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR -> dirs.add(id)
                        isMusic(name) -> music += Child(id, name, if (c.isNull(3)) 0 else c.getLong(3), if (c.isNull(4)) 0 else c.getLong(4))
                        name.endsWith(".lrc", ignoreCase = true) -> lrc[name.substringBeforeLast('.').lowercase()] = id
                        name.lowercase() in COVER_NAMES && cover == null -> cover = id
                    }
                }
            } ?: throw IllegalStateException("the folder can't be listed")
            music.forEach { m ->
                out += Doc(m.id, m.name, m.size, m.modified, lrc[m.name.substringBeforeLast('.').lowercase()], cover)
            }
        }
        return out
    }

    private fun nameOf(tree: Uri): String {
        val doc = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        return runCatching {
            context.contentResolver.query(doc, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            }
        }.getOrNull() ?: DocumentsContract.getTreeDocumentId(tree).substringAfterLast(':').ifBlank { "Music" }
    }

    companion object {
        private const val TAG = "EncoreLocalMusic"
        private const val AUTO_KEY = "look-by-itself"
        private const val PROGRESS_EVERY = 5
        private const val REFRESH_EVERY = 150
        private const val LRC_MAX = 512 * 1024
        private val EXTENSIONS = setOf("mp3", "flac", "m4a", "mp4", "aac", "ogg", "oga", "wav")
        private val COVER_NAMES = setOf("cover.jpg", "cover.png", "folder.jpg", "folder.png", "front.jpg", "front.png", "album.jpg", "albumart.jpg")

        fun isMusic(name: String): Boolean = name.substringAfterLast('.', "").lowercase() in EXTENSIONS
    }
}
