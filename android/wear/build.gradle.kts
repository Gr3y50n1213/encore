import com.android.build.api.variant.ResValue

// Stage W1 (docs/adr/android-0008-wear-os-app.md): Encore for Wear OS watches. It needs the
// phone: everything it shows comes from the phone app over the Wearable Data Layer, which
// connects only apps with the same application ID signed with the same key.
plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose.compiler)
}

val repoRoot: Directory = rootProject.layout.projectDirectory.dir("..")

android {
    namespace = "uk.gcartee.encore.wear"
    compileSdk = 37

    defaultConfig {
        // The phone app's ID: the Data Layer pairs the two by it (and by the signing key).
        applicationId = "uk.gcartee.encore"
        // Wear OS 3 and later (Galaxy Watch 4 and later, Pixel Watch).
        minSdk = 30
        targetSdk = 36
        versionCode = 10000
        versionName = "1.0.0"
    }

    // Signed exactly as the phone app (app/build.gradle.kts): the owner's test key when
    // ENCORE_TEST_KEYSTORE names it, else Android's debug key. The Data Layer only connects
    // the two apps when both carry the same key.
    val testKeystore = providers.environmentVariable("ENCORE_TEST_KEYSTORE").orNull
        ?.let(::file)?.takeIf { it.isFile }
    if (testKeystore != null) {
        signingConfigs.create("test") {
            storeFile = testKeystore
            storePassword = providers.environmentVariable("ENCORE_TEST_KEYSTORE_PASSWORD").get()
            keyAlias = "encore-test"
            keyPassword = providers.environmentVariable("ENCORE_TEST_KEYSTORE_PASSWORD").get()
        }
    }

    buildTypes {
        debug {
            // Pairs with the phone's debug build (uk.gcartee.encore.debug).
            applicationIdSuffix = ".debug"
            signingConfigs.findByName("test")?.let { signingConfig = it }
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (providers.gradleProperty("encore.signReleaseWithDebugKey").orNull == "true") {
                signingConfig = signingConfigs.findByName("test") ?: signingConfigs.getByName("debug")
            }
        }
    }

    buildFeatures {
        compose = true
        resValues = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    lint {
        warningsAsErrors = false
        abortOnError = true
        checkDependencies = false
    }
}

kotlin {
    jvmToolchain(17)
}

// The name people see comes from the core's one brand constant, as in the phone app.
val appName: Provider<String> = providers.fileContents(repoRoot.file("core/encore-core/src/brand.rs")).asText.map { text ->
    Regex("""pub const APP_NAME: &str = "([^"]+)";""").find(text)?.groupValues?.get(1)
        ?: throw GradleException("APP_NAME not found in core/encore-core/src/brand.rs")
}

androidComponents {
    onVariants { variant ->
        // The phone-watch protocol, shared with the phone app.
        variant.sources.kotlin?.addStaticSourceDirectory(rootProject.layout.projectDirectory.dir("wearlink/src").asFile.absolutePath)
        variant.resValues.put(
            variant.makeResValueKey("string", "app_name"),
            appName.map { ResValue(it, "From core/encore-core/src/brand.rs") },
        )
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.coroutines.android)
    implementation(libs.play.services.wearable)
    implementation(libs.wear.compose.material3)
    implementation(libs.wear.compose.foundation)
    implementation(libs.wear.tiles)
    implementation(libs.wear.protolayout)
    implementation(libs.wear.protolayout.material3)
    implementation(libs.wear.complications.data.source.ktx)

    testImplementation(libs.junit)
}
