// Plugin versions for every module. AGP is only put on the build classpath here; `:ui` applies
// it when `-Pvink.ui.nativeTargets=true`, so the JVM preview needs no Android SDK or Xcode.
plugins {
    kotlin("multiplatform") version "2.2.20" apply false
    kotlin("android") version "2.2.20" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.20" apply false
    id("org.jetbrains.compose") version "1.9.0" apply false
    id("com.android.library") version "8.11.1" apply false
    id("com.android.application") version "8.11.1" apply false
}
