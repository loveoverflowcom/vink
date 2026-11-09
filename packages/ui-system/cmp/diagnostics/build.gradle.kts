import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
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
