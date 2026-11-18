package com.jurisgames.tasks.ui

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.temporal.TemporalAdjusters

/**
 * "When" words typed into a new task ("call mum tomorrow at 5"), English and Spanish.
 * [at] is the moment meant, [title] the text without the when-words.
 */
data class When(val at: LocalDateTime, val title: String)

private val WEEKDAYS = mapOf(
    "monday" to DayOfWeek.MONDAY, "tuesday" to DayOfWeek.TUESDAY, "wednesday" to DayOfWeek.WEDNESDAY,
    "thursday" to DayOfWeek.THURSDAY, "friday" to DayOfWeek.FRIDAY, "saturday" to DayOfWeek.SATURDAY, "sunday" to DayOfWeek.SUNDAY,
    "lunes" to DayOfWeek.MONDAY, "martes" to DayOfWeek.TUESDAY, "miercoles" to DayOfWeek.WEDNESDAY, "miércoles" to DayOfWeek.WEDNESDAY,
    "jueves" to DayOfWeek.THURSDAY, "viernes" to DayOfWeek.FRIDAY, "sabado" to DayOfWeek.SATURDAY, "sábado" to DayOfWeek.SATURDAY,
    "domingo" to DayOfWeek.SUNDAY,
)

/** Time of day when only a day is given. */
val MORNING: LocalTime = LocalTime.of(9, 0)

private val day = Regex(
    "(?i)(?<![\\p{L}])(?:(?:on|el)\\s+)?(today|hoy|tonight|esta noche|tomorrow|mañana|manana|next week|la semana que viene|la próxima semana|la proxima semana|" +
        WEEKDAYS.keys.joinToString("|") + ")(?![\\p{L}])",
)
private val clock = Regex(
    "(?i)(?<![\\p{L}\\d])(?:(at|a las|a la)\\s+)?(\\d{1,2})(?::(\\d{2}))?\\s*(am|pm|a\\.m\\.|p\\.m\\.)?(?![\\p{L}\\d])",
)
private val relative = Regex("(?i)(?<![\\p{L}])(?:in|en)\\s+(\\d{1,3})\\s*(min|mins|minutes|minutos|h|hour|hours|hora|horas|day|days|día|dias|días)(?![\\p{L}])")

fun parseWhen(text: String, now: LocalDateTime = LocalDateTime.now()): When? {
    relative.find(text)?.let { m ->
        val n = m.groupValues[1].toLong()
        val u = m.groupValues[2].lowercase()
        val at = when {
            u.startsWith("m") -> now.plusMinutes(n)
            u.startsWith("h") -> now.plusHours(n)
            else -> now.plusDays(n).with(MORNING)
        }
        return When(at.withSecond(0).withNano(0), strip(text, m.range))
    }
    val d = day.find(text)
    var date: LocalDate? = null
    var defaultTime = MORNING
    if (d != null) {
        val w = d.groupValues[1].lowercase()
        date = when (w) {
            "today", "hoy" -> now.toLocalDate()
            "tonight", "esta noche" -> { defaultTime = LocalTime.of(20, 0); now.toLocalDate() }
            "tomorrow", "mañana", "manana" -> now.toLocalDate().plusDays(1)
            "next week", "la semana que viene", "la próxima semana", "la proxima semana" -> now.toLocalDate().with(TemporalAdjusters.next(DayOfWeek.MONDAY))
            else -> now.toLocalDate().with(TemporalAdjusters.next(WEEKDAYS.getValue(w)))
        }
    }
    // A number is a time only with "at"/"a las", am/pm, or minutes ("buy 5 eggs" stays as it is).
    val t = clock.findAll(text).firstOrNull { m ->
        val h = m.groupValues[2].toInt()
        val min = m.groupValues[3]
        (m.groupValues[1].isNotEmpty() || m.groupValues[4].isNotEmpty() || min.isNotEmpty()) && h <= 23 && (min.isEmpty() || min.toInt() < 60) &&
            (d == null || !m.range.overlaps(d.range))
    }
    var time: LocalTime? = null
    if (t != null) {
        var h = t.groupValues[2].toInt()
        val min = t.groupValues[3].ifEmpty { "0" }.toInt()
        val ampm = t.groupValues[4].lowercase()
        if (ampm.startsWith("p") && h < 12) h += 12
        if (ampm.startsWith("a") && h == 12) h = 0
        // "at 5" without am/pm means the afternoon; tonight's "at 9" means 21:00.
        if (ampm.isEmpty() && t.groupValues[3].isEmpty() && h in 1..7) h += 12
        if (ampm.isEmpty() && defaultTime.hour >= 20 && h in 1..11) h += 12
        time = LocalTime.of(h % 24, min)
    }
    if (date == null && time == null) return null
    val at = if (date != null) {
        LocalDateTime.of(date, time ?: defaultTime)
    } else {
        val today = LocalDateTime.of(now.toLocalDate(), time!!)
        if (today.isAfter(now)) today else today.plusDays(1)
    }
    val ranges = listOfNotNull(d?.range, t?.range)
    return When(at, strip(text, *ranges.toTypedArray()))
}

private fun IntRange.overlaps(o: IntRange) = first <= o.last && o.first <= last

private fun strip(text: String, vararg ranges: IntRange): String {
    var s = text
    for (r in ranges.sortedByDescending { it.first }) s = s.removeRange(r)
    return s.replace(Regex("\\s{2,}"), " ").trim()
}

fun LocalDateTime.millis(): Long = atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()

fun fromMillis(ms: Long): LocalDateTime = LocalDateTime.ofInstant(java.time.Instant.ofEpochMilli(ms), ZoneId.systemDefault())

/** "Today 17:00", "Tomorrow 9:00", "Fri 9:00", "12 Dec 9:00": short, for chips and rows. */
fun whenLabel(at: LocalDateTime, now: LocalDateTime = LocalDateTime.now()): String {
    val time = at.toLocalTime().format(DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT))
    val days = java.time.temporal.ChronoUnit.DAYS.between(now.toLocalDate(), at.toLocalDate())
    val dayText = when {
        days == 0L -> "Today"
        days == 1L -> "Tomorrow"
        days == -1L -> "Yesterday"
        // English like the rest of the app until it is translated.
        days in 2..6 -> at.dayOfWeek.getDisplayName(java.time.format.TextStyle.SHORT, java.util.Locale.ENGLISH)
        else -> at.format(DateTimeFormatter.ofPattern("d MMM", java.util.Locale.ENGLISH))
    }
    return "$dayText $time"
}
