pluginManagement {
    repositories {
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        // The Android half of rustls-platform-verifier (TLS certificate checks
        // in the Rust SDK), published only in its own repo.
        exclusiveContent {
            forRepository {
                maven { url = uri("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/") }
            }
            filter { includeModule("org.rustls", "rustls-platform-verifier") }
        }
        google {
            mavenContent {
                includeGroupAndSubgroups("androidx")
                includeGroupAndSubgroups("com.android")
                includeGroupAndSubgroups("com.google")
            }
        }
        mavenCentral()
        // Nightly SDK bindings: published only as GitHub release assets
        // (Element X reads them the same way). Needed for 26.10.x nightlies,
        // which carry the event-cache fixes behind the post-create-room
        // SIGSEGV (matrix-rust-sdk #7204, #7248).
        ivy {
            url = uri("https://github.com/matrix-org/matrix-rust-components-kotlin/releases/download")
            patternLayout { artifact("sdk-[revision]/[artifact]-[revision](-[classifier])(.[ext])") }
            metadataSources { gradleMetadata() }
            content { includeModule("org.matrix.rustcomponents", "sdk-android") }
        }
    }
}

rootProject.name = "fluxchat-companion"
include(":app")
