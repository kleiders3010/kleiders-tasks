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
}

compose.desktop {
    application {
        mainClass = "com.jurisgames.tasks.desktop.MainKt"
    }
}
