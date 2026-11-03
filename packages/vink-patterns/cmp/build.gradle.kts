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
        iosArm64()
        iosSimulatorArm64()
    }
    sourceSets {
        commonMain.dependencies { api(project(":ui")) }
        commonTest.dependencies { implementation(kotlin("test")) }
    }
}

if (nativeTargets) {
    extensions.configure<com.android.build.gradle.LibraryExtension>("android") {
        namespace = "vink.patterns"
        compileSdk = 36
        defaultConfig { minSdk = 24 }
        compileOptions {
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }
    }
}
