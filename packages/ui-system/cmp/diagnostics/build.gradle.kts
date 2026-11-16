import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import groovy.json.JsonSlurper
import java.security.MessageDigest
import java.util.Base64

plugins {
    kotlin("multiplatform")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

// One provenance owner: #258's exact Web assets. No second checked-in image set or manifest.
val emptyAssetDir = rootProject.layout.projectDirectory.dir("../../../apps/vink-web/assets/empty")
val emptySourceDir = layout.buildDirectory.dir("generated/empty-artwork")
val generateEmptyArtwork by tasks.registering {
    inputs.files(emptyAssetDir.asFileTree.matching { include("*.webp", "asset-manifest.json") })
    outputs.dir(emptySourceDir)
    doLast {
        val manifest = JsonSlurper().parse(emptyAssetDir.file("asset-manifest.json").asFile) as Map<*, *>
        val assets = manifest["runtime_assets"] as List<*>
        require(assets.map { (it as Map<*, *>)["id"] }.toSet() == setOf("no-results", "no-public-works", "first-draft"))
        val entries = assets.map { raw ->
            val asset = raw as Map<*, *>
            val id = asset["id"] as String
            require(asset["width"] == 512 && asset["height"] == 427 && asset["alpha"] == true)
            val bytes = emptyAssetDir.file("vink-empty-$id.webp").asFile.readBytes()
            require(bytes.size == asset["bytes"] && bytes.size < 30 * 1024)
            val sha = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
            require(sha == asset["sha256"]) { "Empty artwork hash mismatch: $id" }
            "        \"$id\" -> \"${Base64.getEncoder().encodeToString(bytes)}\""
        }
        val output = emptySourceDir.get().file("vink/ui/preview/EmptyArtworkBytes.kt").asFile
        output.parentFile.mkdirs()
        output.writeText("package vink.ui.preview\n\n// Generated from the canonical #258 assets; do not edit.\ninternal fun emptyArtworkBase64(id: String): String = when (id) {\n" + entries.joinToString("\n") + "\n        else -> error(\"Unknown empty artwork\")\n}\n")
    }
}

val nativeTargets = providers.gradleProperty("vink.ui.nativeTargets").orNull == "true"
if (nativeTargets) apply(plugin = "com.android.library")
kotlin {
    jvm("desktop") { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }
    if (nativeTargets) {
        androidTarget { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }
        listOf(iosArm64(), iosSimulatorArm64()).forEach { target ->
            target.binaries.framework {
                baseName = "VinkDiagnostics"
                isStatic = true
            }
        }
    }
    applyDefaultHierarchyTemplate()
    sourceSets {
        commonMain { kotlin.srcDir(files(emptySourceDir).builtBy(generateEmptyArtwork)) }
        commonMain.dependencies {
            api(project(":ui"))
            api(project(":patterns"))
            implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
        }
        commonTest.dependencies { implementation(kotlin("test")) }
        val skiaMain by creating { dependsOn(commonMain.get()) }
        if (nativeTargets) named("iosMain") { dependsOn(skiaMain) }
        val desktopMain by getting {
            dependsOn(skiaMain)
            dependencies { implementation(compose.desktop.currentOs) }
        }
    }
}
if (nativeTargets) {
    extensions.configure<com.android.build.gradle.LibraryExtension>("android") {
        namespace = "vink.ui.diagnostics"
        compileSdk = 36
        defaultConfig { minSdk = 24 }
        compileOptions {
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }
    }
}
