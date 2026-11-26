// Vink UI system — Compose Multiplatform renderer (`:ui`) and its desktop preview (`:preview`).
// The preview is an app, not a library: its sources live in `apps/vink-preview/cmp` (#552) and
// nothing in `:ui`, `:patterns` or `:diagnostics` may depend on it.
// Versions are pinned here and in gradle/wrapper; see README.md for the compatibility record.
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
        google()
    }
}

rootProject.name = "vink-ui-cmp"
include(":ui", ":preview", ":diagnostics")
if (providers.gradleProperty("vink.ui.nativeTargets").orNull == "true") {
    include(":androidDiagnostic")
}

include(":patterns")
project(":patterns").projectDir = file("../../vink-patterns/cmp")
project(":preview").projectDir = file("../../../apps/vink-preview/cmp")
