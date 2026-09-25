plugins {
    id("org.jetbrains.kotlin.jvm")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.compose")
}

sourceSets["main"].kotlin.srcDir("../shared/src")

dependencies {
    implementation(compose.desktop.currentOs)
    implementation(compose.material3)
    implementation("net.java.dev.jna:jna:5.19.1")
    testImplementation(kotlin("test"))
    testImplementation("org.jetbrains.compose.ui:ui-test-junit4:1.12.1")
    testImplementation("junit:junit:4.13.2")
}

compose.desktop {
    application {
        mainClass = "com.jurisgames.tasks.desktop.MainKt"
    }
}

// Tests use the Rust library built by `cargo build -p kt-ffi --features desktop`.
val rustTarget = rootDir.resolve("../target/debug")
tasks.test {
    systemProperty("jna.library.path", rustTarget.absolutePath)
    systemProperty("kt.worker", rustTarget.resolve(if (System.getProperty("os.name").lowercase().contains("win")) "kt-import-worker.exe" else "kt-import-worker").absolutePath)
}
