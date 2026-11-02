import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

// Android and iOS targets are opt-in so a JVM-only machine builds and previews without the
// Android SDK, Xcode or a Kotlin/Native download. `cargo xtask ui-preview cmp build` passes the
// property when VINK_UI_NATIVE_TARGETS=1.
val nativeTargets = providers.gradleProperty("vink.ui.nativeTargets").orNull == "true"
if (nativeTargets) {
    apply(plugin = "com.android.library")
}

kotlin {
    jvm("desktop") {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
        }
    }
    if (nativeTargets) {
        androidTarget {
            compilerOptions {
                jvmTarget.set(JvmTarget.JVM_17)
            }
        }
        iosArm64()
        iosSimulatorArm64()
    }
    sourceSets {
        commonMain.dependencies {
            api(compose.runtime)
            api(compose.foundation)
            api(compose.ui)
            // The stable Material3 that Compose 1.9 resolves; no Expressive alpha APIs.
            api("org.jetbrains.compose.material3:material3:1.8.2")
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
        val desktopTest by getting {
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation(compose.desktop.uiTestJUnit4)
            }
        }
    }
}

if (nativeTargets) {
    extensions.configure<com.android.build.gradle.LibraryExtension>("android") {
        namespace = "vink.ui"
        compileSdk = 36
        defaultConfig {
            minSdk = 24
        }
        compileOptions {
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }
    }
}
