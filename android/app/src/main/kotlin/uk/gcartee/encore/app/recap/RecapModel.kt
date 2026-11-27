package uk.gcartee.encore.app.recap

/**
 * The recap data document the core's `recap` call returns (schema 1,
 * docs/core/recap-engine.md), as small Kotlin types. Unknown fields are ignored and a beat
 * of a type this app does not know becomes [BeatData.Unknown], which the story leaves out,
 * so a newer core never breaks an older app.
 */
data class Recap(
    val schema: Int,
    val engine: Int,
    val year: Int,
    val kind: RecapKind,
    val noneReason: String?,
    val soFar: Boolean,
    val locked: Boolean,
    val countedThroughUtcMs: Long,
    val since: RecapSince?,
    val pack: RecapPack?,
    val seed: String,
    val palette: RecapPalette?,
    val yearLine: String?,
    val totals: RecapTotals,
    val beats: List<Beat>,
    val detail: RecapDetail?,
    val logHash: String,
) {
    /** The beats this app draws, in story order. */
    val story: List<Beat> get() = beats.filter { it.data !is BeatData.Unknown }
}

enum class RecapKind { NONE, SHORT, FULL }

data class RecapSince(val day: String, val reason: String)

data class RecapPack(val id: String, val version: Int)

data class RecapPalette(
    val source: String,
    val colours: List<String>,
    val lead: String,
    val leadHue: Int,
    val hueGapFromLastYear: Int?,
)

data class RecapTotals(
    val plays: Long,
    val listenedMs: Long,
    val minutes: Long,
    val songs: Long,
    val artists: Long,
    val albums: Long,
    val activeDays: Long,
)

data class Beat(
    val id: String,
    val role: String,
    val fixed: Boolean,
    val seconds: Int,
    val layout: Int,
    val layoutCount: Int,
    val shapeSeed: Long,
    val score: Long?,
    val template: String,
    val text: String,
    val data: BeatData,
    val math: BeatMath,
)

data class BeatMath(val counts: List<MathCount>, val rule: String)

data class MathCount(val label: String, val value: Long, val unit: String)

data class SongEntry(val rank: Int, val key: String, val title: String, val artist: String, val album: String?, val plays: Long, val listenedMs: Long)

data class ArtistEntry(val rank: Int, val key: String, val name: String, val plays: Long, val listenedMs: Long)

data class AlbumEntry(val rank: Int, val key: String, val name: String, val artist: String, val plays: Long, val listenedMs: Long)

data class GenreEntry(val rank: Int, val name: String, val plays: Long, val permille: Long)

data class MonthTally(val month: Int, val plays: Long, val minutes: Long)

data class Metric(val id: String, val label: String, val value: Long, val unit: String)

data class Season(val id: String, val months: List<Int>, val song: SongEntry)

data class SoundPoint(val month: Int, val energy: Long?, val valence: Long?)

data class Drift(val genre: String, val afterMonth: Int, val risePermille: Long)

data class RecapDetail(
    val topSongs: List<SongEntry>,
    val topArtists: List<ArtistEntry>,
    val topAlbums: List<AlbumEntry>,
    val months: List<MonthTally>,
    val personality: List<Metric>,
)

/** The numbers one card draws, by beat type (`data.type`). */
sealed interface BeatData {
    data class Intro(val year: Int, val yearLine: String, val albumKeys: List<String>) : BeatData
    data class Minutes(
        val minutes: Long,
        val days: Long,
        val hours: Long,
        val months: List<MonthTally>,
        val lastYearMinutes: Long?,
        val changePercent: Long?,
        val peakPart: String?,
    ) : BeatData
    data class TopSong(val top: List<SongEntry>, val snippetStartMs: Long?) : BeatData
    data class TopArtist(val top: List<ArtistEntry>) : BeatData
    data class TopAlbum(val top: List<AlbumEntry>, val playThroughs: Long?) : BeatData
    data class YourSound(val top: List<GenreEntry>, val months: List<List<Long>>, val drift: Drift?) : BeatData
    data class Personality(
        val archetype: String,
        val name: String,
        val metric: String,
        val value: Long,
        val threshold: Long,
        val unit: String,
        val lastYearValue: Long?,
        val metrics: List<Metric>,
    ) : BeatData
    data class Closer(val title: String, val songKeys: List<String>, val count: Long, val durationMs: Long, val durationKnown: Long) : BeatData
    data class ListeningClock(val peakWeekday: Int, val peakHour: Int, val grid: List<Long>, val peakPart: String, val topWeekday: Int) : BeatData
    data class BiggestDiscovery(val artist: ArtistEntry, val firstDay: String, val firstMonth: Int, val months: List<Long>) : BeatData
    data class Comeback(val artist: ArtistEntry, val playsBefore: Long, val gapDays: Long) : BeatData
    data class ObsessionWeek(val artist: ArtistEntry, val weekStart: String, val days: List<Long>, val weekPlays: Long, val weekSharePermille: Long) : BeatData
    data class LongestStreak(val days: Long, val firstDay: String, val lastDay: String) : BeatData
    data class SongOfEachSeason(val seasons: List<Season>) : BeatData
    data class MostReplayedDay(val song: SongEntry, val day: String, val plays: Long) : BeatData
    data class MoodArc(val months: List<SoundPoint>, val firstHalfEnergy: Long, val secondHalfEnergy: Long) : BeatData
    data class MostSungLine(val song: SongEntry, val lineIndex: Int, val lineStartMs: Long, val jumps: Long) : BeatData
    /** A beat type this app does not draw (a newer core); left out of the story. */
    data class Unknown(val type: String) : BeatData
}

/** Reads the core's recap JSON. Throws [IllegalArgumentException] when it is not a recap document. */
fun parseRecap(json: String): Recap {
    val root = JsonObj.of(RecapJson.parse(json)) ?: throw IllegalArgumentException("The recap is not a JSON object")
    val beats = root.objs("beats").map(::readBeat)
    val kind = when (root.str("kind")) {
        "none" -> RecapKind.NONE
        "short" -> RecapKind.SHORT
        "full" -> RecapKind.FULL
        // A kind a newer core added: draw what it gives.
        else -> if (beats.isEmpty()) RecapKind.NONE else RecapKind.FULL
    }
    return Recap(
        schema = root.int("schema") ?: 0,
        engine = root.int("engine") ?: 0,
        year = root.int("year") ?: throw IllegalArgumentException("The recap has no year"),
        kind = kind,
        noneReason = root.str("noneReason"),
        soFar = root.bool("soFar") ?: false,
        locked = root.bool("locked") ?: false,
        countedThroughUtcMs = root.long("countedThroughUtcMs") ?: 0L,
        since = root.obj("since")?.let { s -> s.str("day")?.let { RecapSince(it, s.str("reason").orEmpty()) } },
        pack = root.obj("pack")?.let { p -> p.str("id")?.let { RecapPack(it, p.int("version") ?: 1) } },
        seed = root.str("seed").orEmpty(),
        palette = root.obj("palette")?.let { p ->
            RecapPalette(
                source = p.str("source").orEmpty(),
                colours = p.strs("colours"),
                lead = p.str("lead").orEmpty(),
                leadHue = p.int("leadHue") ?: 0,
                hueGapFromLastYear = p.int("hueGapFromLastYear"),
            )
        },
        yearLine = root.str("yearLine"),
        totals = root.obj("totals").let { t ->
            RecapTotals(
                plays = t?.long("plays") ?: 0L,
                listenedMs = t?.long("listenedMs") ?: 0L,
                minutes = t?.long("minutes") ?: 0L,
                songs = t?.long("songs") ?: 0L,
                artists = t?.long("artists") ?: 0L,
                albums = t?.long("albums") ?: 0L,
                activeDays = t?.long("activeDays") ?: 0L,
            )
        },
        beats = beats,
        detail = root.obj("detail")?.let { d ->
            RecapDetail(
                topSongs = d.objs("topSongs").map(::readSong),
                topArtists = d.objs("topArtists").map(::readArtist),
                topAlbums = d.objs("topAlbums").map(::readAlbum),
                months = d.objs("months").map(::readMonth),
                personality = d.objs("personality").map(::readMetric),
            )
        },
        logHash = root.str("logHash").orEmpty(),
    )
}

private fun readBeat(b: JsonObj): Beat {
    val dataObj = b.obj("data")
    val type = dataObj?.str("type").orEmpty()
    // A field missing from a known type (a broken or future document) leaves the card out
    // instead of drawing a wrong number.
    val data = dataObj?.let { runCatching { readData(type, it) }.getOrNull() } ?: BeatData.Unknown(type)
    return Beat(
        id = b.str("id").orEmpty(),
        role = b.str("role").orEmpty(),
        fixed = b.bool("fixed") ?: false,
        seconds = b.int("seconds") ?: 0,
        layout = b.int("layout") ?: 0,
        layoutCount = b.int("layoutCount") ?: 1,
        shapeSeed = b.long("shapeSeed") ?: 0L,
        score = b.long("score"),
        template = b.str("template").orEmpty(),
        text = b.str("text").orEmpty(),
        data = data,
        math = b.obj("math").let { m ->
            BeatMath(
                counts = m?.objs("counts")?.map { c -> MathCount(c.str("label").orEmpty(), c.long("value") ?: 0L, c.str("unit").orEmpty()) }.orEmpty(),
                rule = m?.str("rule").orEmpty(),
            )
        },
    )
}

private fun JsonObj.need(k: String): Long = long(k) ?: throw IllegalArgumentException("missing $k")
private fun JsonObj.needStr(k: String): String = str(k) ?: throw IllegalArgumentException("missing $k")
private fun JsonObj.needObj(k: String): JsonObj = obj(k) ?: throw IllegalArgumentException("missing $k")

private fun readData(type: String, d: JsonObj): BeatData = when (type) {
    "intro" -> BeatData.Intro(d.need("year").toInt(), d.str("yearLine").orEmpty(), d.strs("albumKeys"))
    "minutes" -> BeatData.Minutes(
        minutes = d.need("minutes"),
        days = d.long("days") ?: 0L,
        hours = d.long("hours") ?: 0L,
        months = d.objs("months").map(::readMonth),
        lastYearMinutes = d.long("lastYearMinutes"),
        changePercent = d.long("changePercent"),
        peakPart = d.str("peakPart"),
    )
    "topSong" -> BeatData.TopSong(d.objs("top").map(::readSong).also { require(it.isNotEmpty()) }, d.long("snippetStartMs"))
    "topArtist" -> BeatData.TopArtist(d.objs("top").map(::readArtist).also { require(it.isNotEmpty()) })
    "topAlbum" -> BeatData.TopAlbum(d.objs("top").map(::readAlbum).also { require(it.isNotEmpty()) }, d.long("playThroughs"))
    "yourSound" -> BeatData.YourSound(
        top = d.objs("top").map { g -> GenreEntry(g.int("rank") ?: 0, g.str("name").orEmpty(), g.long("plays") ?: 0L, g.long("permille") ?: 0L) },
        months = d.list("months").map { row -> (row as? List<*>)?.map { (it as? Number)?.toLong() ?: 0L }.orEmpty() },
        drift = d.obj("drift")?.let { Drift(it.str("genre").orEmpty(), it.int("afterMonth") ?: 0, it.long("risePermille") ?: 0L) },
    )
    "personality" -> BeatData.Personality(
        archetype = d.str("archetype").orEmpty(),
        name = d.needStr("name"),
        metric = d.str("metric").orEmpty(),
        value = d.need("value"),
        threshold = d.long("threshold") ?: 0L,
        unit = d.str("unit").orEmpty(),
        lastYearValue = d.long("lastYearValue"),
        metrics = d.objs("metrics").map(::readMetric),
    )
    "closer" -> BeatData.Closer(d.str("title").orEmpty(), d.strs("songKeys"), d.long("count") ?: 0L, d.long("durationMs") ?: 0L, d.long("durationKnown") ?: 0L)
    "listeningClock" -> BeatData.ListeningClock(
        peakWeekday = d.need("peakWeekday").toInt(),
        peakHour = d.need("peakHour").toInt(),
        grid = d.longs("grid"),
        peakPart = d.str("peakPart").orEmpty(),
        topWeekday = d.int("topWeekday") ?: 0,
    )
    "biggestDiscovery" -> BeatData.BiggestDiscovery(readArtist(d.needObj("artist")), d.str("firstDay").orEmpty(), d.int("firstMonth") ?: 0, d.longs("months"))
    "comeback" -> BeatData.Comeback(readArtist(d.needObj("artist")), d.long("playsBefore") ?: 0L, d.long("gapDays") ?: 0L)
    "obsessionWeek" -> BeatData.ObsessionWeek(
        artist = readArtist(d.needObj("artist")),
        weekStart = d.str("weekStart").orEmpty(),
        days = d.longs("days"),
        weekPlays = d.long("weekPlays") ?: 0L,
        weekSharePermille = d.long("weekSharePermille") ?: 0L,
    )
    "longestStreak" -> BeatData.LongestStreak(d.need("days"), d.str("firstDay").orEmpty(), d.str("lastDay").orEmpty())
    "songOfEachSeason" -> BeatData.SongOfEachSeason(
        d.objs("seasons").map { s -> Season(s.str("id").orEmpty(), s.longs("months").map { it.toInt() }, readSong(s.needObj("song"))) }
            .also { require(it.isNotEmpty()) },
    )
    "mostReplayedDay" -> BeatData.MostReplayedDay(readSong(d.needObj("song")), d.str("day").orEmpty(), d.need("plays"))
    "moodArc" -> BeatData.MoodArc(
        months = d.objs("months").map { p -> SoundPoint(p.int("month") ?: 0, p.long("energy"), p.long("valence")) },
        firstHalfEnergy = d.long("firstHalfEnergy") ?: 0L,
        secondHalfEnergy = d.long("secondHalfEnergy") ?: 0L,
    )
    "mostSungLine" -> BeatData.MostSungLine(readSong(d.needObj("song")), d.int("lineIndex") ?: 0, d.long("lineStartMs") ?: 0L, d.need("jumps"))
    else -> BeatData.Unknown(type)
}

private fun readSong(s: JsonObj) = SongEntry(
    rank = s.int("rank") ?: 0,
    key = s.str("key").orEmpty(),
    title = s.str("title").orEmpty(),
    artist = s.str("artist").orEmpty(),
    album = s.str("album"),
    plays = s.long("plays") ?: 0L,
    listenedMs = s.long("listenedMs") ?: 0L,
)

private fun readArtist(a: JsonObj) = ArtistEntry(a.int("rank") ?: 0, a.str("key").orEmpty(), a.str("name").orEmpty(), a.long("plays") ?: 0L, a.long("listenedMs") ?: 0L)

private fun readAlbum(a: JsonObj) = AlbumEntry(a.int("rank") ?: 0, a.str("key").orEmpty(), a.str("name").orEmpty(), a.str("artist").orEmpty(), a.long("plays") ?: 0L, a.long("listenedMs") ?: 0L)

private fun readMonth(m: JsonObj) = MonthTally(m.int("month") ?: 0, m.long("plays") ?: 0L, m.long("minutes") ?: 0L)

private fun readMetric(m: JsonObj) = Metric(m.str("id").orEmpty(), m.str("label").orEmpty(), m.long("value") ?: 0L, m.str("unit").orEmpty())
