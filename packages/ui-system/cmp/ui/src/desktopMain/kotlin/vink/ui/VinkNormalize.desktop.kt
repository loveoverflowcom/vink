package vink.ui

import java.text.Normalizer

/** JVM targets: the platform NFC normaliser. */
actual fun nfc(text: String): String = Normalizer.normalize(text, Normalizer.Form.NFC)
