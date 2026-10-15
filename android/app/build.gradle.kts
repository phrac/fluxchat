plugins {
    alias(libs.plugins.androidApplication)
    alias(libs.plugins.composeCompiler)
}

android {
    namespace = "org.fluxchat.companion"
    compileSdk = 37

    defaultConfig {
        applicationId = "org.fluxchat.companion"
        minSdk = 28
        targetSdk = 36
        versionCode = 8
        versionName = "0.2.6"
    }

    signingConfigs {
        create("release") {
            System.getenv("KEYSTORE_PATH")?.let { storeFile = file(it) }
            storePassword = System.getenv("STORE_PASSWORD")
            keyAlias = System.getenv("KEY_ALIAS")
            keyPassword = System.getenv("KEY_PASSWORD")
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Signed with your own key when KEYSTORE_PATH etc. are set, else the
            // debug key (fine for installing on your own phone).
            signingConfig = if (System.getenv("KEYSTORE_PATH") != null) signingConfigs.getByName("release") else signingConfigs.getByName("debug")
        }
    }

    // Phones and the emulator; the 32-bit libraries would add ~100 MB.
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "x86_64")
            isUniversalApk = false
        }
    }

    buildFeatures {
        compose = true
        // Update.kt reads VERSION_NAME for the release check (off by default in AGP 8).
        buildConfig = true
    }

    testOptions {
        unitTests.isIncludeAndroidResources = true
    }

    // The harness profiles are the desktop's (assets/agents/*.toml), so both
    // read agents the same way.
    sourceSets {
        getByName("main") {
            assets.directories.add(layout.buildDirectory.dir("generated/harness-assets").get().asFile.path)
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}

val copyHarnessProfiles = tasks.register<Copy>("copyHarnessProfiles") {
    from(rootProject.layout.projectDirectory.dir("../assets/agents"))
    include("*.toml")
    into(layout.buildDirectory.dir("generated/harness-assets/agents"))
}

tasks.named("preBuild") {
    dependsOn(copyHarnessProfiles)
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
    implementation(libs.compose.tooling.preview)
    debugImplementation(libs.compose.tooling)
    implementation(libs.core.ktx)
    implementation(libs.splashscreen)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime)
    implementation(libs.lifecycle.viewmodel)
    implementation(libs.lifecycle.process)
    implementation(libs.coroutines.android)
    implementation(libs.matrix.sdk)
    implementation(libs.rustls.platform.verifier)
    implementation(libs.unifiedpush)
    implementation(libs.biometric)
    // biometric 1.1.0 drags fragment 1.1.0, which trips InvalidFragmentVersionForActivityResult
    // on our registerForActivityResult calls; pin a current one.
    implementation(libs.fragment)
    implementation(libs.tomlj)

    testImplementation(libs.junit)
    testImplementation(libs.json)
    // Screenshots of the UI with sample data (Robolectric): ./gradlew :app:testDebugUnitTest -Droborazzi.test.record=true
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(libs.robolectric)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test)
    debugImplementation(libs.compose.ui.test.manifest)
}

tasks.withType<Test>().configureEach {
    systemProperty("roborazzi.test.record", System.getProperty("roborazzi.test.record") ?: "false")
}
