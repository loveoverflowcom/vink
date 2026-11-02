import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

// The self-hosted faces are copied from the web app's asset directory at build time (with their
// OFL licences and provenance), never duplicated in Git.
val syncPreviewFonts by tasks.registering(Sync::class) {
    from(rootProject.layout.projectDirectory.dir("../../../apps/vink-web/assets/fonts")) {
        include("*.woff2", "OFL-*.txt", "provenance.json")
    }
    into(layout.buildDirectory.dir("generated/preview-fonts/vink-fonts"))
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
                implementation(compose.desktop.currentOs)
            }
            resources.srcDir(files(layout.buildDirectory.dir("generated/preview-fonts")).builtBy(syncPreviewFonts))
        }
        val desktopTest by getting {
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
