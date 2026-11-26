import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

// Desktop uses losslessly decompressed TTF derivatives: Linux's pinned Skia cannot decode
// these WOFF2s. The web asset provenance owns both formats, their hashes and OFL licences.
val syncPreviewFonts by tasks.registering(Sync::class) {
    from(layout.projectDirectory.dir("fonts")) { include("*.ttf") }
    from(rootProject.layout.projectDirectory.dir("../../../apps/vink-web/assets/fonts")) {
        include("OFL-*.txt", "provenance.json")
    }
    into(layout.buildDirectory.dir("generated/preview-fonts/vink-fonts"))
}

// Keep the original inputs in the test classpath for format/decoder diagnostics only.
val syncFontProbeSources by tasks.registering(Sync::class) {
    from(rootProject.layout.projectDirectory.dir("../../../apps/vink-web/assets/fonts")) { include("*.woff2") }
    into(layout.buildDirectory.dir("generated/font-probe/vink-web-fonts"))
}

kotlin {
    jvm("desktop") {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
        }
    }
    sourceSets {
        val desktopMain by getting {
            dependencies {
                implementation(project(":ui"))
                implementation(project(":patterns"))
                implementation(project(":diagnostics"))
                implementation(compose.desktop.currentOs)
            }
            resources.srcDir(files(layout.buildDirectory.dir("generated/preview-fonts")).builtBy(syncPreviewFonts))
        }
        val desktopTest by getting {
            resources.srcDir(files(layout.buildDirectory.dir("generated/font-probe")).builtBy(syncFontProbeSources))
            dependencies {
                implementation(kotlin("test"))
                implementation(kotlin("test-junit"))
                implementation(compose.desktop.uiTestJUnit4)
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "vink.ui.preview.MainKt"
    }
}

tasks.withType<Test>().configureEach {
    // Captures land here for a person to open; the run is headless.
    systemProperty("vink.captures", layout.buildDirectory.dir("captures").get().asFile.absolutePath)
    systemProperty("java.awt.headless", "true")
}
