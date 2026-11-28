package uk.gcartee.encore.app.pebble

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.core.content.ContextCompat
import androidx.core.content.IntentCompat
import java.util.UUID
import uk.gcartee.encore.app.playback.PlaybackHub
import uk.gcartee.encore.app.wear.WearCommands
import uk.gcartee.encore.wearlink.NowPlaying

/**
 * Stage W2 (docs/adr/android-0009-pebble.md): the phone's side of the Encore watchapp on a
 * Pebble, through the Pebble app's PebbleKit broadcasts ([Pebble], [PebbleProtocol.kt]).
 *
 * - It runs only with a Pebble app installed (Core Devices' or the original, checked with
 *   PackageManager; the manifest's `<queries>` names both), and sends only while the
 *   watchapp is open: the watchapp says hello when it opens, and the phone stops when its
 *   messages go unanswered ([ACK_TIMEOUT_MS]), until the watch says something again.
 * - Now Playing comes from [uk.gcartee.encore.app.wear.WearLink], which builds it once for
 *   both watches, on the same changes and with the same limit of two sends a second.
 * - A send names the Pebble app's package, so no other app on the phone receives the
 *   titles. What the watch sends back (a button) comes as the Pebble app's broadcast to
 *   every app, so the receiver is registered at run time while Encore runs: Android
 *   doesn't deliver such a broadcast to a receiver in the manifest.
 * - Each message from the watch is ACKed (PebbleKit asks that every one is) and mapped to
 *   the Wear OS watch's commands, so the buttons do what the Wear watch's do ([WearCommands]).
 */
object PebbleLink {
    private const val TAG = "PebbleLink"

    /** Without an ACK by then, the watchapp is taken to be closed. */
    private const val ACK_TIMEOUT_MS = 10_000L

    /** A NACK (the watch was busy) is retried this many times before giving up. */
    private const val MAX_RETRIES = 2
    private const val RETRY_MS = 1_000L
    private const val MAX_OUTSTANDING = 16

    private val main = Handler(Looper.getMainLooper())
    private val lock = Any()

    @Volatile private var attached = false
    @Volatile private var packages: List<String> = emptyList()
    @Volatile private var onOpened: () -> Unit = {}

    /** The watchapp is open and answering: Now Playing is sent. */
    @Volatile var active = false
        private set

    // Under [lock]: the last state the watch has (or was sent), and the sends not answered yet.
    private var lastSent: NowPlaying? = null
    private var lastView: PebbleNow.View? = null
    private var counter = 0
    private var latest = -1
    private val outstanding = ArrayDeque<Int>()
    private var retries = 0

    private val timeout = Runnable { gaveUp("no answer") }

    /**
     * From [uk.gcartee.encore.app.wear.WearLink.attach]. Returns whether a Pebble app is
     * installed; [opened] runs when the watchapp says hello, to send it everything.
     */
    fun attach(context: Context, opened: () -> Unit): Boolean {
        if (attached) return packages.isNotEmpty()
        attached = true
        val found = installed(context.packageManager)
        packages = found
        if (found.isEmpty()) return false
        onOpened = opened
        val filter = IntentFilter().apply {
            addAction(Pebble.ACTION_RECEIVE)
            addAction(Pebble.ACTION_RECEIVE_ACK)
            addAction(Pebble.ACTION_RECEIVE_NACK)
        }
        // Exported: the Pebble app is another app. What it can do here is what the buttons
        // do (play, pause, skip and like the song playing), checked against the UUID.
        runCatching { ContextCompat.registerReceiver(context, Receiver(), filter, ContextCompat.RECEIVER_EXPORTED) }
            .onFailure {
                Log.w(TAG, "No receiver for the Pebble app: ${it.javaClass.simpleName}")
                return false
            }
        return true
    }

    private fun installed(pm: PackageManager): List<String> = Pebble.PEBBLE_APPS.filter { name ->
        runCatching {
            if (Build.VERSION.SDK_INT >= 33) {
                pm.getPackageInfo(name, PackageManager.PackageInfoFlags.of(0))
            } else {
                @Suppress("DEPRECATION")
                pm.getPackageInfo(name, 0)
            }
        }.isSuccess
    }

    /**
     * Sends [now] when the watch's view of it changed (the position alone moving on never
     * does). Returns whether it sent. From WearLink's send loop.
     */
    fun send(context: Context, now: NowPlaying): Boolean {
        if (!active) return false
        val view = PebbleNow.view(now)
        val id: Int
        val startTimer: Boolean
        synchronized(lock) {
            lastSent = now
            if (view == lastView) return false
            lastView = view
            counter = Pebble.transactionId(counter + 1)
            id = counter
            latest = id
            retries = 0
            // The clock runs from the oldest send not answered, so steady sends to a closed
            // watchapp still stop.
            startTimer = outstanding.isEmpty()
            outstanding.addLast(id)
            while (outstanding.size > MAX_OUTSTANDING) outstanding.removeFirst()
        }
        broadcast(context, id, view)
        if (startTimer) {
            main.removeCallbacks(timeout)
            main.postDelayed(timeout, ACK_TIMEOUT_MS)
        }
        return true
    }

    private fun broadcast(context: Context, id: Int, view: PebbleNow.View) {
        val json = PebbleDictionary.toJson(view.tuples())
        for (pkg in packages) {
            val intent = Intent(Pebble.ACTION_SEND)
                .setPackage(pkg)
                .putExtra(Pebble.EXTRA_UUID, Pebble.APP_UUID)
                .putExtra(Pebble.EXTRA_TRANSACTION_ID, id)
                .putExtra(Pebble.EXTRA_MSG_DATA, json)
            runCatching { context.sendBroadcast(intent) }
                .onFailure { Log.w(TAG, "Couldn't reach the Pebble app: ${it.javaClass.simpleName}") }
        }
    }

    private fun ack(context: Context, action: String, id: Int) {
        for (pkg in packages) {
            val intent = Intent(action).setPackage(pkg).putExtra(Pebble.EXTRA_TRANSACTION_ID, id)
            runCatching { context.sendBroadcast(intent) }
        }
    }

    private fun gaveUp(why: String) {
        synchronized(lock) {
            if (!active) return
            active = false
            outstanding.clear()
            lastView = null
        }
        main.removeCallbacks(timeout)
        Log.i(TAG, "The Pebble stopped answering ($why); waiting for it to say hello")
    }

    /** The Pebble app's RECEIVE_ACK or RECEIVE_NACK for one of the phone's sends. */
    private fun onAnswer(context: Context, id: Int, ok: Boolean) {
        var resend: PebbleNow.View? = null
        synchronized(lock) {
            if (id !in outstanding) return
            if (ok) {
                // The watchapp is open and took one: it's there.
                outstanding.clear()
                retries = 0
            } else if (id != latest) {
                // An older state refused; a newer one is on its way.
                outstanding.remove(id)
                return
            } else if (retries < MAX_RETRIES) {
                retries++
                resend = lastView
            }
        }
        if (ok) {
            main.removeCallbacks(timeout)
            return
        }
        val view = resend ?: return gaveUp("refused")
        main.postDelayed({ broadcast(context, id, view) }, RETRY_MS)
    }

    private fun onMessage(context: Context, intent: Intent) {
        val uuid = IntentCompat.getSerializableExtra(intent, Pebble.EXTRA_UUID, UUID::class.java)
            ?: intent.getStringExtra(Pebble.EXTRA_UUID)?.let { runCatching { UUID.fromString(it) }.getOrNull() }
        // Another watchapp's message: not ours to ACK (PebbleKit's PebbleDataReceiver, too).
        if (uuid != Pebble.APP_UUID) return
        val id = intent.getIntExtra(Pebble.EXTRA_TRANSACTION_ID, -1)
        val command = PebbleDictionary.fromJson(intent.getStringExtra(Pebble.EXTRA_MSG_DATA))?.let(PebbleCommand::of)
        ack(context, if (command != null) Pebble.ACTION_ACK else Pebble.ACTION_NACK, id)
        if (command == null) {
            Log.w(TAG, "Ignored a message that isn't a command")
            return
        }
        val opened: Boolean
        val known: NowPlaying?
        synchronized(lock) {
            opened = !active || command == PebbleCommand.HELLO
            active = true
            if (opened) {
                // Send it everything, fresh.
                lastView = null
                outstanding.clear()
                retries = 0
            }
            known = lastSent
        }
        if (opened) onOpened()
        // Play or pause by the player's own state; like or unlike by what the watch was shown.
        val playing = PlaybackHub.isConnected.value?.state?.value?.isPlaying ?: (known?.isPlaying == true)
        val wear = command.toWear(isPlaying = playing, isLiked = known?.liked == true) ?: return
        WearCommands.run(context.applicationContext, wear)
    }

    private class Receiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                Pebble.ACTION_RECEIVE -> onMessage(context, intent)
                Pebble.ACTION_RECEIVE_ACK -> onAnswer(context, intent.getIntExtra(Pebble.EXTRA_TRANSACTION_ID, -1), ok = true)
                Pebble.ACTION_RECEIVE_NACK -> onAnswer(context, intent.getIntExtra(Pebble.EXTRA_TRANSACTION_ID, -1), ok = false)
            }
        }
    }
}
