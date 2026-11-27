package uk.gcartee.encore.app.recap

/**
 * A small JSON reader for the recap data document (docs/core/recap-engine.md). Plain Kotlin,
 * so the JVM unit tests read the core's golden files with the same code the app uses:
 * Android's org.json is only a stub there, and the app adds no JSON library.
 *
 * Objects become maps (in document order), arrays lists, numbers Long (or Double when they
 * have a fraction or an exponent), and true, false and null themselves.
 */
internal object RecapJson {
    class Error(message: String) : IllegalArgumentException(message)

    fun parse(text: String): Any? {
        val r = Reader(text)
        r.space()
        val v = r.value(0)
        r.space()
        if (r.i != text.length) throw Error("Unexpected text at ${r.i}")
        return v
    }

    private const val MAX_DEPTH = 64

    private class Reader(val s: String) {
        var i = 0

        fun peek(): Char = if (i < s.length) s[i] else '\u0000'

        fun space() {
            while (i < s.length && (s[i] == ' ' || s[i] == '\n' || s[i] == '\r' || s[i] == '\t')) i++
        }

        fun expect(c: Char) {
            if (peek() != c) throw Error("Expected '$c' at $i")
            i++
        }

        fun value(depth: Int): Any? {
            if (depth > MAX_DEPTH) throw Error("Nested too deep")
            if (i >= s.length) throw Error("Unexpected end")
            val c = s[i]
            return when {
                c == '{' -> obj(depth)
                c == '[' -> arr(depth)
                c == '"' -> str()
                c == 't' -> word("true", true)
                c == 'f' -> word("false", false)
                c == 'n' -> word("null", null)
                c == '-' || c in '0'..'9' -> num()
                else -> throw Error("Unexpected '$c' at $i")
            }
        }

        fun word(w: String, v: Any?): Any? {
            if (!s.startsWith(w, i)) throw Error("Bad literal at $i")
            i += w.length
            return v
        }

        fun obj(depth: Int): Map<String, Any?> {
            val m = LinkedHashMap<String, Any?>()
            expect('{')
            space()
            if (peek() == '}') {
                i++
                return m
            }
            while (true) {
                space()
                val k = str()
                space()
                expect(':')
                space()
                m[k] = value(depth + 1)
                space()
                val c = peek()
                if (c == ',') {
                    i++
                } else if (c == '}') {
                    i++
                    return m
                } else {
                    throw Error("Expected ',' or '}' at $i")
                }
            }
        }

        fun arr(depth: Int): List<Any?> {
            val l = ArrayList<Any?>()
            expect('[')
            space()
            if (peek() == ']') {
                i++
                return l
            }
            while (true) {
                space()
                l.add(value(depth + 1))
                space()
                val c = peek()
                if (c == ',') {
                    i++
                } else if (c == ']') {
                    i++
                    return l
                } else {
                    throw Error("Expected ',' or ']' at $i")
                }
            }
        }

        fun str(): String {
            expect('"')
            val b = StringBuilder()
            while (true) {
                if (i >= s.length) throw Error("Unterminated string")
                val c = s[i++]
                if (c == '"') return b.toString()
                if (c != '\\') {
                    b.append(c)
                    continue
                }
                if (i >= s.length) throw Error("Bad escape")
                val e = s[i++]
                when (e) {
                    '"' -> b.append('"')
                    '\\' -> b.append('\\')
                    '/' -> b.append('/')
                    'b' -> b.append('\b')
                    'f' -> b.append('\u000C')
                    'n' -> b.append('\n')
                    'r' -> b.append('\r')
                    't' -> b.append('\t')
                    'u' -> {
                        if (i + 4 > s.length) throw Error("Bad escape")
                        val code = s.substring(i, i + 4).toIntOrNull(16) ?: throw Error("Bad escape at $i")
                        b.append(code.toChar())
                        i += 4
                    }
                    else -> throw Error("Bad escape at $i")
                }
            }
        }

        fun num(): Any {
            val start = i
            if (peek() == '-') i++
            while (i < s.length && (s[i] in '0'..'9' || s[i] == '.' || s[i] == 'e' || s[i] == 'E' || s[i] == '+' || s[i] == '-')) i++
            val t = s.substring(start, i)
            val real = t.any { it == '.' || it == 'e' || it == 'E' }
            if (!real) t.toLongOrNull()?.let { return it }
            return t.toDoubleOrNull() ?: throw Error("Bad number '$t' at $start")
        }
    }
}

/** Typed reads from one parsed JSON object; a missing or mistyped field reads as null or empty. */
internal class JsonObj(private val m: Map<String, Any?>) {
    fun str(k: String): String? = m[k] as? String
    fun long(k: String): Long? = (m[k] as? Number)?.toLong()
    fun int(k: String): Int? = long(k)?.toInt()
    fun bool(k: String): Boolean? = m[k] as? Boolean
    fun obj(k: String): JsonObj? = of(m[k])
    fun list(k: String): List<Any?> = (m[k] as? List<*>) ?: emptyList()
    fun objs(k: String): List<JsonObj> = list(k).mapNotNull(::of)
    fun longs(k: String): List<Long> = list(k).map { (it as? Number)?.toLong() ?: 0L }
    fun strs(k: String): List<String> = list(k).mapNotNull { it as? String }

    companion object {
        fun of(v: Any?): JsonObj? {
            val map = v as? Map<*, *> ?: return null
            val out = LinkedHashMap<String, Any?>()
            for ((k, value) in map) if (k is String) out[k] = value
            return JsonObj(out)
        }
    }
}
