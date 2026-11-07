pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // The Kotlin half of rustls-platform-verifier (request core-0001). Its project
        // publishes it only here, as a Maven folder in its own repository; nothing
        // else may come from this source.
        exclusiveContent {
            forRepository {
                maven {
                    name = "rustlsPlatformVerifier"
                    url = uri("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
                }
            }
            filter { includeModule("org.rustls", "rustls-platform-verifier") }
        }
    }
}

rootProject.name = "encore-android"
include(":app")
