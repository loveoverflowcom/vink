plugins {
    id("com.android.application")
    kotlin("android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "vink.ui.diagnostic"
    compileSdk = 36
    defaultConfig {
        applicationId = "com.vietink.ui.diagnostic"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-diagnostic"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    // Diagnostic debug host only; no signed release or production distribution.
    buildTypes { release { isMinifyEnabled = false } }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
kotlin { compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) } }
androidComponents {
    beforeVariants(selector().withBuildType("release")) { it.enable = false }
}

dependencies {
    implementation(project(":diagnostics"))
    implementation("androidx.activity:activity-compose:1.10.1")
    androidTestImplementation("androidx.compose.ui:ui-test-junit4:1.9.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.7.0")
}
