plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.jurisgames.tasks"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }
    defaultConfig {
        applicationId = "com.jurisgames.tasks"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-prototype"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    buildFeatures { compose = true }
    // play: Google Play (Block Store, code scanner, ...); foss: no proprietary Play libraries
    // (F-Droid later). The engine has no Play Services dependency either way (DESIGN §12).
    flavorDimensions += "store"
    productFlavors {
        create("play") { dimension = "store" }
        create("foss") { dimension = "store" }
    }
    buildTypes {
        release {
            // R8: what Play users download. Signing happens at release time, outside the repo.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    // Native libraries stay as files: the import sandbox worker is executed from there.
    packaging { jniLibs { useLegacyPackaging = true } }
    sourceSets["main"].kotlin.directories.add("../shared/src")
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    val bom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(bom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.work:work-runtime-ktx:2.12.0")
    implementation("androidx.glance:glance-appwidget:1.2.0")
    // The code scanner brings an old Fragment that breaks registerForActivityResult (lint).
    implementation("androidx.fragment:fragment-ktx:1.9.1")
    // QR scanning: Google's code scanner needs no camera permission; foss scans with the camera.
    add("playImplementation", "com.google.android.gms:play-services-code-scanner:16.1.0")
    add("fossImplementation", "androidx.camera:camera-camera2:1.6.2")
    add("fossImplementation", "androidx.camera:camera-lifecycle:1.6.2")
    add("fossImplementation", "androidx.camera:camera-view:1.6.2")
    add("fossImplementation", "com.google.zxing:core:3.5.4")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    androidTestImplementation(bom)
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.glance:glance-appwidget-testing:1.2.0")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
