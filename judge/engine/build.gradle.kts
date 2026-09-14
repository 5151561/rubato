plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.parcelize)
    alias(libs.plugins.ksp)
}

android {
    namespace = "io.legado.app"
    compileSdk = 37

    defaultConfig {
        minSdk = 26
    }

    buildFeatures {
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }

    kotlin {
        jvmToolchain(21)
    }
}

dependencies {
    api(project(":rhino"))

    api(libs.okhttp)
    api(libs.jsoup)
    api(libs.jsoupxpath)
    api(libs.json.path)
    api(libs.gson)
    api(libs.commons.text)
    api(libs.kotlinx.coroutines.android)
    api(libs.androidx.collection)
    api(libs.androidx.annotation)
    api(libs.androidx.core.ktx)
    api(libs.splitties.appctx)
    api(libs.splitties.systemservices)
    api(libs.androidx.documentfile)
    api(libs.androidx.lifecycle.runtime.ktx)

    api(libs.room.runtime)
    api(libs.room.ktx)
    ksp(libs.room.compiler)

    implementation(libs.libarchive)
    implementation(libs.quick.chinese.transfer)
}
