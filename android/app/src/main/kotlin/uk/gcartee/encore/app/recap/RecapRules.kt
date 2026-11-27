package uk.gcartee.encore.app.recap

import java.text.NumberFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Story
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Taps
import uk.gcartee.encore.tokens.EncoreTokens.Recap.Timeline

/**
 * The story's rules (board 39, items 4 and 6; plan section 25, "Story rules"), as plain
 * Kotlin so they are tested on the JVM. The numbers come from design/tokens.json `recap`.
 */
object RecapRules {
    enum class Tap { BACK, FORWARD }

    /** Item 6: the left 35 % goes back, the right 65 % forward. */
    fun tapZone(x: Float, width: Float): Tap =
        if (width > 0f && x < width * Taps.backZonePercent / 100f) Tap.BACK else Tap.FORWARD

    /** Item 6, GUARD: the first tap within 300 ms of a card appearing is ignored. */
    fun tapCounts(sinceCardShownMs: Long): Boolean = sinceCardShownMs >= Taps.firstTapIgnoredMs

    /** How long a card stays: the beat's own seconds (6 to 8, a list card 9), kept in that range. */
    fun cardMillis(seconds: Int): Long =
        seconds.coerceIn(Story.CardSeconds.min, maxOf(Story.listCardSeconds, Story.CardSeconds.max)).toLong() * 1000L

    /** Item 4: the rest of a top 5 shows 2 s after the card. */
    const val REVEAL_REST_MS: Long = Story.top5RevealAfterSeconds * 1000L

    private const val DAY_MS = 24L * 60L * 60L * 1000L

    /**
     * Item 6: swipe down closes, and the story resumes where it left off for 30 days. A
     * finished story, or one left longer ago, starts again at the first card.
     */
    fun resumeIndex(saved: Int, savedAtMs: Long, nowMs: Long, finished: Boolean, count: Int): Int {
        if (count <= 0 || finished || saved <= 0) return 0
        val age = nowMs - savedAtMs
        if (age < 0 || age > Taps.resumeWithinDays * DAY_MS) return 0
        return saved.coerceIn(0, count - 1)
    }

    private fun monthDay(token: String): Pair<Int, Int> {
        val parts = token.split('-')
        return (parts.getOrNull(0)?.toIntOrNull() ?: 1) to (parts.getOrNull(1)?.toIntOrNull() ?: 1)
    }

    /**
     * Board 41, "When it is ready": which year's story Home offers on [day]. From 1 December
     * this year's ("so far"), and until 8 January last year's (the final run); else none.
     */
    fun seasonYear(day: LocalDate): Int? {
        val (fromMonth, fromDay) = monthDay(Timeline.soFarFrom)
        val (lockMonth, lockDay) = monthDay(Timeline.lockedFrom)
        return when {
            day.monthValue == fromMonth && day.dayOfMonth >= fromDay -> day.year
            day.monthValue == lockMonth && day.dayOfMonth <= lockDay -> day.year - 1
            else -> null
        }
    }

    /** Whether [day] is in the final run (1 to 8 January), when late plays may still arrive. */
    fun inFinalRun(day: LocalDate): Boolean {
        val (lockMonth, lockDay) = monthDay(Timeline.lockedFrom)
        return day.monthValue == lockMonth && day.dayOfMonth <= lockDay
    }

    /**
     * The seasons turn round south of the equator. The device's region is enough: countries
     * wholly or mostly in the southern hemisphere, plus a southern time zone.
     */
    fun southern(country: String, timeZoneId: String): Boolean =
        country.uppercase(Locale.ROOT) in SOUTHERN ||
            timeZoneId.startsWith("Australia/") || timeZoneId.startsWith("Antarctica/") ||
            timeZoneId == "Pacific/Auckland" || timeZoneId == "Pacific/Chatham"

    private val SOUTHERN = setOf(
        "AR", "AU", "BO", "BR", "BW", "CL", "FJ", "LS", "MG", "MU", "MW", "MZ", "NA", "NC", "NZ",
        "PE", "PY", "SZ", "TO", "UY", "VU", "WS", "ZA", "ZM", "ZW", "AO", "PF", "CK", "FK", "RE",
    )
}

/** Numbers and dates as the cards write them. */
object RecapFormat {
    private val MONTHS = listOf("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
    private val MONTHS_LONG = listOf("January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December")
    private val WEEKDAYS = listOf("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")
    val MONTH_LETTERS = listOf("J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D")
    val WEEKDAY_LETTERS = listOf("M", "T", "W", "T", "F", "S", "S")

    fun count(n: Long): String = NumberFormat.getIntegerInstance(Locale.getDefault()).format(n)

    /** A permille as a whole percent, rounded half up (recap-engine.md, "Units"). */
    fun percent(permille: Long): Long = if (permille >= 0) (permille + 5) / 10 else -((-permille + 5) / 10)

    fun hour(h: Int): String = when {
        h <= 0 || h >= 24 -> "12 am"
        h < 12 -> "$h am"
        h == 12 -> "12 pm"
        else -> "${h - 12} pm"
    }

    /** "12a", "6a", "12p", "6p": the hour grid's axis. */
    fun hourShort(h: Int): String = when {
        h % 24 == 0 -> "12a"
        h < 12 -> "${h}a"
        h == 12 -> "12p"
        else -> "${h - 12}p"
    }

    fun weekday(mondayFirst: Int): String = WEEKDAYS.getOrElse(mondayFirst) { "" }

    fun month(m: Int): String = MONTHS.getOrElse(m - 1) { "" }

    fun monthLong(m: Int): String = MONTHS_LONG.getOrElse(m - 1) { "" }

    /** "2026-03-02" as "Mar 2"; anything else as it is. */
    fun day(iso: String): String = runCatching {
        val d = LocalDate.parse(iso)
        "${month(d.monthValue)} ${d.dayOfMonth}"
    }.getOrDefault(iso)

    /** "6 h 41 m", or "41 m" under an hour. */
    fun hoursMinutes(ms: Long): String {
        val minutes = ms / 60_000L
        val h = minutes / 60
        val m = minutes % 60
        return if (h > 0) "$h h $m m" else "$m m"
    }

    fun hours(ms: Long): Long = (ms + 30L * 60_000L) / 3_600_000L

    /** The local day the counting reached, never past 31 December of the recap year. */
    fun countedThrough(recap: Recap, zone: ZoneId): String {
        val end = LocalDate.of(recap.year, 12, 31)
        val reached = runCatching { Instant.ofEpochMilli(recap.countedThroughUtcMs).atZone(zone).toLocalDate() }.getOrDefault(end)
        val d = if (reached.isAfter(end)) end else reached
        return "${month(d.monthValue)} ${d.dayOfMonth}"
    }

    /** A Show-the-math count with its unit ("1,349 plays", "25%"). */
    fun mathValue(c: MathCount): String = when (c.unit) {
        "plays" -> "${count(c.value)} ${if (c.value == 1L) "play" else "plays"}"
        "minutes" -> "${count(c.value)} ${if (c.value == 1L) "minute" else "minutes"}"
        "days" -> "${count(c.value)} ${if (c.value == 1L) "day" else "days"}"
        "permille" -> "${percent(c.value)}%"
        "degrees" -> "${c.value}°"
        "ms" -> hoursMinutes(c.value)
        else -> count(c.value)
    }

    fun metricValue(m: Metric): String = if (m.unit == "permille") "${percent(m.value)}%" else count(m.value)

    fun seasonName(id: String): String = id.replaceFirstChar { it.titlecase(Locale.ROOT) }
}

/**
 * Board 39, item 11: every card has a text version, read in order, and every chart has its
 * values as text. Built from the core's own sentence and numbers, never new claims.
 */
object RecapText {
    fun title(beatId: String): String = when (beatId) {
        "intro" -> "Your year in music"
        "minutes" -> "Minutes"
        "topSong" -> "Top song"
        "topArtist" -> "Top artist"
        "topAlbum" -> "Top album"
        "yourSound" -> "Your sound"
        "personality" -> "Your listening personality"
        "closer" -> "Yours to keep"
        "listeningClock" -> "Listening clock"
        "biggestDiscovery" -> "Biggest discovery"
        "comeback" -> "Comeback"
        "obsessionWeek" -> "Obsession week"
        "longestStreak" -> "Longest streak"
        "songOfEachSeason" -> "Song of each season"
        "mostReplayedDay" -> "Most replayed day"
        "moodArc" -> "Mood arc"
        "mostSungLine" -> "Most-sung line"
        else -> "Your year"
    }

    private fun c(n: Long) = RecapFormat.count(n)

    private fun plays(n: Long) = "${c(n)} ${if (n == 1L) "play" else "plays"}"

    /** The card's words for a screen reader: position, title, numbers, chart values, the date. */
    fun describe(recap: Recap, beat: Beat, index: Int, count: Int, countedThrough: String): String {
        val parts = mutableListOf("Card ${index + 1} of $count.", "${title(beat.id)}.")
        when (val d = beat.data) {
            is BeatData.Intro -> {
                parts += "Your ${d.year}."
                if (d.yearLine.isNotBlank()) parts += d.yearLine
            }
            is BeatData.Minutes -> {
                parts += "${c(d.minutes)} minutes with your music, ${d.days} days and ${d.hours} hours."
                parts += beat.text
                parts += "By month: " + d.months.joinToString(", ") { "${RecapFormat.monthLong(it.month)} ${c(it.minutes)} minutes" } + "."
            }
            is BeatData.TopSong -> {
                val one = d.top.first()
                parts += "${one.title} by ${one.artist}, ${plays(one.plays)}."
                parts += beat.text
                if (d.top.size > 1) parts += "Then " + d.top.drop(1).joinToString("; ") { "${it.rank}, ${it.title} by ${it.artist}, ${plays(it.plays)}" } + "."
            }
            is BeatData.TopArtist -> {
                val one = d.top.first()
                parts += "${one.name}, ${plays(one.plays)}, ${RecapFormat.hours(one.listenedMs)} hours."
                parts += beat.text
                if (d.top.size > 1) parts += "Then " + d.top.drop(1).joinToString("; ") { "${it.rank}, ${it.name}, ${plays(it.plays)}" } + "."
            }
            is BeatData.TopAlbum -> {
                val one = d.top.first()
                parts += "${one.name} by ${one.artist}, ${plays(one.plays)}."
                d.playThroughs?.takeIf { it > 0 }?.let { parts += "Played start to finish ${c(it)} ${if (it == 1L) "time" else "times"}." }
                parts += beat.text
                if (d.top.size > 1) parts += "Then " + d.top.drop(1).joinToString("; ") { "${it.rank}, ${it.name} by ${it.artist}, ${plays(it.plays)}" } + "."
            }
            is BeatData.YourSound -> {
                parts += beat.text
                parts += d.top.joinToString(", ") { "${it.name} ${RecapFormat.percent(it.permille)}%" } + "."
            }
            is BeatData.Personality -> {
                parts += "You are ${d.name}."
                parts += beat.text
                parts += d.metrics.joinToString(", ") { "${it.label} ${RecapFormat.metricValue(it)}" } + "."
            }
            is BeatData.Closer -> {
                parts += "${d.title}, ${c(d.count)} songs, ${RecapFormat.hoursMinutes(d.durationMs)}."
                parts += beat.text
            }
            is BeatData.ListeningClock -> {
                parts += "Busiest hour ${RecapFormat.hour(d.peakHour)}, busiest at ${RecapFormat.weekday(d.peakWeekday)}, ${RecapFormat.hour(d.peakHour)}."
                parts += beat.text
                val byHour = (0 until 24).map { h -> (0 until 7).sumOf { w -> d.grid.getOrElse(w * 24 + h) { 0L } } }
                parts += "Plays by hour: " + byHour.mapIndexed { h, v -> "${RecapFormat.hour(h)} ${c(v)}" }.joinToString(", ") + "."
            }
            is BeatData.BiggestDiscovery -> {
                parts += "${d.artist.name}, ${plays(d.artist.plays)}."
                parts += beat.text
                parts += "By month: " + d.months.mapIndexed { i, v -> "${RecapFormat.monthLong(i + 1)} ${c(v)}" }.joinToString(", ") + "."
            }
            is BeatData.Comeback -> {
                parts += "${d.artist.name}: ${plays(d.playsBefore)} before, back after ${c(d.gapDays)} days, ${plays(d.artist.plays)} this year."
                parts += beat.text
            }
            is BeatData.ObsessionWeek -> {
                parts += beat.text
                parts += "${plays(d.weekPlays)} in seven days: " + d.days.mapIndexed { i, v -> "${RecapFormat.weekday(i)} ${c(v)}" }.joinToString(", ") + "."
            }
            is BeatData.LongestStreak -> {
                parts += "${c(d.days)} days in a row, ${RecapFormat.day(d.firstDay)} to ${RecapFormat.day(d.lastDay)}."
                parts += beat.text
            }
            is BeatData.SongOfEachSeason -> {
                parts += beat.text
                parts += d.seasons.joinToString("; ") { "${RecapFormat.seasonName(it.id)}, ${it.song.title} by ${it.song.artist}" } + "."
            }
            is BeatData.MostReplayedDay -> {
                parts += "${d.song.title} by ${d.song.artist}, ${plays(d.plays)} on ${RecapFormat.day(d.day)}."
                parts += beat.text
            }
            is BeatData.MoodArc -> {
                parts += beat.text
                parts += "Energy by month: " + d.months.filter { it.energy != null }
                    .joinToString(", ") { "${RecapFormat.monthLong(it.month)} ${RecapFormat.percent(it.energy ?: 0L)}%" } + "."
            }
            is BeatData.MostSungLine -> {
                parts += "${c(d.jumps)} times back to line ${d.lineIndex + 1} of ${d.song.title} by ${d.song.artist}."
                parts += beat.text
            }
            is BeatData.Unknown -> parts += beat.text
        }
        if (recap.soFar) parts += "So far."
        parts += "Counted through $countedThrough."
        recap.since?.let { parts += "Since ${RecapFormat.day(it.day)}." }
        return parts.filter { it.isNotBlank() }.joinToString(" ")
    }

    /** Every song, artist and album name in the recap, for hiding them on a shared card. */
    fun names(recap: Recap): Set<String> {
        val out = mutableSetOf<String>()
        fun song(s: SongEntry) {
            out += s.title
            out += s.artist
            s.album?.let { out += it }
        }
        for (b in recap.beats) when (val d = b.data) {
            is BeatData.TopSong -> d.top.forEach(::song)
            is BeatData.TopArtist -> d.top.forEach { out += it.name }
            is BeatData.TopAlbum -> d.top.forEach { out += it.name; out += it.artist }
            is BeatData.BiggestDiscovery -> out += d.artist.name
            is BeatData.Comeback -> out += d.artist.name
            is BeatData.ObsessionWeek -> out += d.artist.name
            is BeatData.SongOfEachSeason -> d.seasons.forEach { song(it.song) }
            is BeatData.MostReplayedDay -> song(d.song)
            is BeatData.MostSungLine -> song(d.song)
            else -> Unit
        }
        recap.detail?.let { det ->
            det.topSongs.forEach(::song)
            det.topArtists.forEach { out += it.name }
            det.topAlbums.forEach { out += it.name; out += it.artist }
        }
        return out.filter { it.isNotBlank() }.toSet()
    }

    const val HIDDEN = "•••"

    /** [text] with every one of [names] replaced, longest first so "A B" goes before "A". */
    fun mask(text: String, names: Set<String>): String {
        var out = text
        for (n in names.sortedByDescending { it.length }) out = out.replace(n, HIDDEN, ignoreCase = true)
        return out
    }
}
