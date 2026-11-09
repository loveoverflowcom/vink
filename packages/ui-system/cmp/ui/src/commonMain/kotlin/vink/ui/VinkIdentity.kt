package vink.ui

/**
 * `avatar-identity-v1`, the same contract as Leptos `vink_ui::identity`; both renderers test
 * the golden vectors in VinkIdentityVectors.kt. Nothing here looks up a profile, an email or an
 * image, and a list position never chooses a colour.
 */

/** Unicode NFC; each platform supplies its normaliser (commonMain has none). */
expect fun nfc(text: String): String

/** Letters (BMP: surrogate halves are not letters) and ASCII digits start an initial. */
private fun startsInitial(c: Char): Boolean = c in '0'..'9' || c.isLetter()

private fun isMark(c: Char): Boolean = when (c.category) {
    CharCategory.NON_SPACING_MARK, CharCategory.COMBINING_SPACING_MARK, CharCategory.ENCLOSING_MARK -> true
    else -> false
}

private fun initial(word: String): String? {
    val start = word.indexOfFirst(::startsInitial)
    if (start < 0) return null
    var end = start + 1
    while (end < word.length && isMark(word[end])) end++
    return word.substring(start, end)
}

/** Words split on Unicode whitespace, like Rust `split_whitespace`. */
private fun words(text: String): List<String> {
    val out = mutableListOf<String>()
    val current = StringBuilder()
    for (c in text) {
        if (c.isWhitespace()) {
            if (current.isNotEmpty()) out += current.toString()
            current.clear()
        } else {
            current.append(c)
        }
    }
    if (current.isNotEmpty()) out += current.toString()
    return out
}

/**
 * First and last word of the NFC display name, uppercased; null means show the neutral mark.
 * "An Nhiên" → "AN", "Nguyễn" → "N", blank or emoji-only → null.
 */
fun initials(displayName: String?): String? {
    val letters = words(nfc(displayName ?: return null)).mapNotNull(::initial)
    val first = letters.firstOrNull() ?: return null
    return (if (letters.size > 1) first + letters.last() else first).uppercase()
}

/** Tone `1..TONES` from a stable identity key (never an email or list index); null if absent. */
fun tone(identityKey: String?): Int? {
    if (identityKey.isNullOrEmpty()) return null
    var hash = 0x811c9dc5u
    for (byte in nfc(identityKey).encodeToByteArray()) {
        hash = (hash xor byte.toUByte().toUInt()) * 0x01000193u
    }
    return 1 + (hash % VinkIdentityVectors.TONES.toUInt()).toInt()
}

/** Background and content roles of a tone; null is the neutral tone. */
fun avatarRoles(tone: Int?): Pair<ColorRole, ColorRole> = when (tone) {
    1 -> ColorRole.Avatar1 to ColorRole.OnAvatar1
    2 -> ColorRole.Avatar2 to ColorRole.OnAvatar2
    3 -> ColorRole.Avatar3 to ColorRole.OnAvatar3
    4 -> ColorRole.Avatar4 to ColorRole.OnAvatar4
    5 -> ColorRole.Avatar5 to ColorRole.OnAvatar5
    6 -> ColorRole.Avatar6 to ColorRole.OnAvatar6
    else -> ColorRole.AvatarNeutral to ColorRole.OnAvatarNeutral
}

/** A person in a group: shown in the caller's order, coloured by [key]. */
data class Person(val key: String?, val name: String?, val image: ImageState = ImageState.Absent)

/** "+N" only when the total is known and larger than what is shown; never a guess or "+0". */
fun overflow(shown: Int, total: Int?): Int? = total?.takeIf { it > shown }?.minus(shown)

/** The one accessible name of a group; same wording as Leptos `group_summary`. */
fun groupSummary(label: String, people: List<Person>, maxVisible: Int, total: Int?): String {
    val names = people.take(maxVisible).map { p -> p.name?.takeIf { it.isNotBlank() } ?: "một người chưa có tên" }
    val more = overflow(names.size, total)
    return when {
        more != null -> "$label: ${names.joinToString(", ")} và $more người khác"
        names.isNotEmpty() -> "$label: ${names.joinToString(", ")}"
        total == 0 -> "$label: chưa có ai"
        else -> "$label: chưa rõ"
    }
}

private val IMAGE_DATA = listOf("png", "jpeg", "webp", "gif").map { "data:image/$it;base64," }

/**
 * The Leptos link allow-list, shared so both renderers refuse the same targets: relative URLs and
 * http(s)/mailto pass; `javascript:`, `data:`, `//host`, whitespace or control characters do not.
 */
fun safeHref(href: String): String? {
    val h = href.trim()
    if (h.isEmpty() || h.startsWith("//") || h.any { it.isWhitespace() || it.isISOControl() }) return null
    val end = h.indexOfAny(charArrayOf('/', '?', '#')).let { if (it < 0) h.length else it }
    val colon = h.substring(0, end).indexOf(':')
    if (colon < 0) return h
    val scheme = h.substring(0, colon).lowercase()
    return h.takeIf { scheme in setOf("http", "https", "mailto") }
}

/** The Leptos image-source allow-list, for adapters that resolve URLs before decoding. */
fun safeImageSrc(src: String): String? {
    val s = src.trim()
    if (s.isEmpty() || s.any { it.isWhitespace() || it.isISOControl() }) return null
    val lower = s.lowercase()
    val ok = (s.startsWith("/") && !s.startsWith("//")) || lower.startsWith("https://") ||
        lower.startsWith("http://") || lower.startsWith("blob:") || IMAGE_DATA.any { lower.startsWith(it) }
    return s.takeIf { ok }
}
