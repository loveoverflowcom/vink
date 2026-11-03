// Generated from the shared Unicode conformance fixture. Do not edit.
package vink.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class NotoExpressionTest {
    @Test fun exactSequenceLookupAndUnicodeFallback() {
        assertEquals("noto-smile", NotoExpressions.lookup("😀")?.id)
        assertEquals("noto-joy", NotoExpressions.lookup("😂")?.id)
        assertEquals("noto-hearts-face", NotoExpressions.lookup("🥰")?.id)
        assertEquals("noto-thinking", NotoExpressions.lookup("🤔")?.id)
        assertEquals("noto-cry", NotoExpressions.lookup("😢")?.id)
        assertEquals("noto-hug", NotoExpressions.lookup("🤗")?.id)
        assertEquals("noto-thanks", NotoExpressions.lookup("🙏")?.id)
        assertEquals("noto-thumbs-up", NotoExpressions.lookup("👍")?.id)
        assertEquals("noto-thumbs-up-medium", NotoExpressions.lookup("👍🏽")?.id)
        assertEquals("noto-clap", NotoExpressions.lookup("👏")?.id)
        assertEquals("noto-party", NotoExpressions.lookup("🎉")?.id)
        assertEquals("noto-sparkles", NotoExpressions.lookup("✨")?.id)
        assertEquals("noto-heart", NotoExpressions.lookup("❤️")?.id)
        assertEquals("noto-heart", NotoExpressions.lookup("❤")?.id)
        assertEquals("noto-heart-fire", NotoExpressions.lookup("❤️‍🔥")?.id)
        assertEquals("noto-heart-fire", NotoExpressions.lookup("❤‍🔥")?.id)
        assertEquals("noto-fire", NotoExpressions.lookup("🔥")?.id)
        assertEquals("noto-wink", NotoExpressions.lookup("😉")?.id)
        assertEquals(null, NotoExpressions.lookup("👍🏻")?.id)
        assertEquals(null, NotoExpressions.lookup("👩‍👩‍👧‍👦")?.id)
        assertEquals(null, NotoExpressions.lookup("🇻🇳")?.id)
        assertEquals(null, NotoExpressions.lookup("1️⃣")?.id)
        assertEquals(null, NotoExpressions.lookup("1⃣")?.id)
        assertEquals(null, NotoExpressions.lookup("☃️")?.id)
        assertEquals(null, NotoExpressions.lookup("😀😀")?.id)
        assertEquals(null, NotoExpressions.lookup("😀‍")?.id)
        assertEquals(null, NotoExpressions.lookup("á — thơ\n  vẫn còn")?.id)
        assertEquals(null, NotoExpressions.lookup("")?.id)
        assertEquals(16, NotoExpressions.assets.size)
    }
}
