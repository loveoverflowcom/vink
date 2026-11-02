package vink.ui

import kotlinx.cinterop.BetaInteropApi
import platform.Foundation.NSString
import platform.Foundation.create
import platform.Foundation.precomposedStringWithCanonicalMapping

/** iOS: Foundation's canonical precomposition is Unicode NFC. */
@OptIn(BetaInteropApi::class)
actual fun nfc(text: String): String = NSString.create(string = text).precomposedStringWithCanonicalMapping
