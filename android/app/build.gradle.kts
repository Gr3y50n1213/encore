import com.android.build.api.variant.ResValue
import javax.inject.Inject
import org.gradle.process.ExecOperations

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose.compiler)
}

// The repository root: android/ sits beside core/.
val repoRoot: Directory = rootProject.layout.projectDirectory.dir("..")
val coreRoot: Directory = repoRoot.dir("core")

android {
    namespace = "uk.gcartee.encore.app"
    // Material 3 1.5 compiles against API 37; the app still targets 36 (plan section 19).
    compileSdk = 37
    ndkVersion = libs.versions.ndk.get()

    defaultConfig {
        // docs/adr/core-0001-names-and-identifiers.md: the owner's domain, reversed.
        applicationId = "uk.gcartee.encore"
        // Plan section 15: Android 9. Section 19: target Android 16 (API 36).
        minSdk = 28
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        // Package only the ABIs the core is built for: JNA's aar also carries armeabi
        // and MIPS libraries, and a device must never get an ABI without the core.
        ndk {
            abiFilters += providers.gradleProperty("encore.abis").get()
                .split(',').map(String::trim).filter(String::isNotEmpty)
        }
    }

    // One test key for every build that goes on the owner's phones, so a new build
    // installs over the last one. Never the Play key (that is the owner's, made before
    // the first upload). It lives outside the repository: ENCORE_TEST_KEYSTORE names
    // the file (CI decodes it from a repository secret). Without it, Android's debug key.
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
            applicationIdSuffix = ".debug"
            signingConfigs.findByName("test")?.let { signingConfig = it }
        }
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // No release key exists yet (it is the owner's, made before the first Play
            // upload). For trying a release build on a phone or in CI, pass
            // -Pencore.signReleaseWithDebugKey=true: it is signed with the test key above
            // when there is one, else the debug key. Never for a build that leaves the house.
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

    packaging {
        jniLibs {
            // Store native libraries uncompressed and page aligned, as Android 15+ 16 KB
            // devices load them straight from the APK.
            useLegacyPackaging = false
        }
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

// --- The Rust core, built for Android inside android/ (CLAUDE.md: each platform builds
// the core for its own targets; the Rust source stays in core/). ---

/** Builds libencore_core.so for each ABI with the NDK's clang, 16 KB page aligned. */
abstract class BuildRustCore @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val coreSources: ConfigurableFileCollection

    @get:Internal
    abstract val coreDir: DirectoryProperty

    @get:Internal
    abstract val ndkDir: DirectoryProperty

    @get:Input
    abstract val ndkVersion: Property<String>

    @get:Input
    abstract val abis: ListProperty<String>

    @get:Input
    abstract val minApi: Property<Int>

    @get:Internal
    abstract val cargoTargetDir: DirectoryProperty

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun build() {
        val os = System.getProperty("os.name").lowercase()
        val host = when {
            os.contains("windows") -> "windows-x86_64"
            os.contains("mac") -> "darwin-x86_64"
            else -> "linux-x86_64"
        }
        val exe = if (os.contains("windows")) ".cmd" else ""
        val bin = ndkDir.get().dir("toolchains/llvm/prebuilt/$host/bin").asFile
        require(bin.isDirectory) { "NDK ${ndkVersion.get()} has no $bin; install it with sdkmanager \"ndk;${ndkVersion.get()}\"" }
        val out = outputDir.get().asFile
        out.deleteRecursively()
        for (abi in abis.get()) {
            val triple = when (abi) {
                "arm64-v8a" -> "aarch64-linux-android"
                "armeabi-v7a" -> "armv7-linux-androideabi"
                "x86_64" -> "x86_64-linux-android"
                "x86" -> "i686-linux-android"
                else -> throw GradleException("Unknown ABI $abi")
            }
            val clangTriple = if (abi == "armeabi-v7a") "armv7a-linux-androideabi" else triple
            val clang = File(bin, "$clangTriple${minApi.get()}-clang$exe").absolutePath
            val upper = triple.uppercase().replace('-', '_')
            val lower = triple.replace('-', '_')
            exec.exec {
                workingDir = coreDir.get().asFile
                environment("CARGO_TARGET_DIR", cargoTargetDir.get().asFile.absolutePath)
                environment("CARGO_TARGET_${upper}_LINKER", clang)
                environment("CC_$lower", clang)
                environment("AR_$lower", File(bin, "llvm-ar${if (exe.isEmpty()) "" else ".exe"}").absolutePath)
                // 16 KB pages: the NDK r28 default, passed explicitly as core CI does
                // (developer.android.com/guide/practices/page-sizes).
                environment(
                    "CARGO_TARGET_${upper}_RUSTFLAGS",
                    "-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384",
                )
                commandLine(
                    "cargo", "rustc", "--locked", "--release", "-p", "encore-core",
                    "--target", triple, "--crate-type", "cdylib",
                )
            }
            val so = cargoTargetDir.get().file("$triple/release/libencore_core.so").asFile
            so.copyTo(File(out, "$abi/libencore_core.so"), overwrite = true)
        }
    }
}

/** Copies prebuilt libraries (-Pencore.jniLibs=<dir>) so the app can skip the Rust build. */
abstract class CopyPrebuiltCore : DefaultTask() {
    @get:InputDirectory
    abstract val from: DirectoryProperty

    @get:Input
    abstract val abis: ListProperty<String>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun copy() {
        val out = outputDir.get().asFile
        out.deleteRecursively()
        for (abi in abis.get()) {
            val so = from.get().file("$abi/libencore_core.so").asFile
            require(so.isFile) { "No $so in the prebuilt folder" }
            so.copyTo(File(out, "$abi/libencore_core.so"), overwrite = true)
        }
    }
}

val abiList: List<String> = providers.gradleProperty("encore.abis").get()
    .split(',').map(String::trim).filter(String::isNotEmpty)
val prebuilt: String = providers.gradleProperty("encore.jniLibs").orNull.orEmpty().trim()

val coreLibs: TaskProvider<out DefaultTask> = if (prebuilt.isNotEmpty()) {
    tasks.register<CopyPrebuiltCore>("copyPrebuiltCore") {
        from.set(file(prebuilt))
        abis.set(abiList)
        outputDir.set(layout.buildDirectory.dir("rust/jniLibs"))
    }
} else {
    tasks.register<BuildRustCore>("buildRustCore") {
        description = "Builds libencore_core.so from core/ for $abiList"
        coreSources.from(
            fileTree(coreRoot) {
                include("Cargo.toml", "Cargo.lock", "*/Cargo.toml", "*/build.rs", "*/src/**", "*/uniffi.toml")
                exclude("target/**")
            },
            repoRoot.file("design/tokens.json"),
        )
        coreDir.set(coreRoot)
        ndkDir.set(androidComponents.sdkComponents.ndkDirectory)
        ndkVersion.set(libs.versions.ndk)
        abis.set(abiList)
        minApi.set(28)
        cargoTargetDir.set(layout.buildDirectory.dir("cargo"))
        outputDir.set(layout.buildDirectory.dir("rust/jniLibs"))
    }
}

/**
 * The verifier's Kotlin part must be the version the core's Rust part expects
 * (rustls-platform-verifier README, "Android"): fail early when core/Cargo.lock moves.
 */
abstract class CheckVerifierVersion : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val cargoLock: RegularFileProperty

    @get:Input
    abstract val expected: Property<String>

    @get:OutputFile
    abstract val stamp: RegularFileProperty

    @TaskAction
    fun check() {
        val lines = cargoLock.get().asFile.readLines()
        val at = lines.indexOfFirst { it.trim() == "name = \"rustls-platform-verifier-android\"" }
        require(at >= 0) { "rustls-platform-verifier-android is not in core/Cargo.lock" }
        val version = lines[at + 1].substringAfter('"').substringBefore('"')
        if (version != expected.get()) {
            throw GradleException(
                "core/Cargo.lock has rustls-platform-verifier-android $version but " +
                    "gradle/libs.versions.toml pins ${expected.get()}: update rustlsPlatformVerifier",
            )
        }
        stamp.get().asFile.writeText(version)
    }
}

val checkVerifierVersion = tasks.register<CheckVerifierVersion>("checkVerifierVersion") {
    cargoLock.set(coreRoot.file("Cargo.lock"))
    expected.set(libs.versions.rustlsPlatformVerifier)
    stamp.set(layout.buildDirectory.file("checks/verifier-version.txt"))
}
tasks.named("preBuild") { dependsOn(checkVerifierVersion) }

/** Board 17, About and licenses: the app's and the core's licence lists, shipped as an asset. */
abstract class LicenceAssets : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val sources: ConfigurableFileCollection

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun write() {
        val out = File(outputDir.get().asFile, "licenses").apply { mkdirs() }
        File(out, "THIRD_PARTY_LICENSES.md").writeText(sources.files.sortedBy { it.path }.joinToString("\n\n") { it.readText() })
    }
}

val licenceAssets = tasks.register<LicenceAssets>("licenceAssets") {
    sources.from(rootProject.layout.projectDirectory.file("THIRD_PARTY_LICENSES.md"), coreRoot.file("THIRD_PARTY_LICENSES.md"))
    outputDir.set(layout.buildDirectory.dir("generated/licenceAssets"))
}

// The name people see comes from the core's one brand constant (core/encore-core/src/brand.rs).
val appName: Provider<String> = providers.fileContents(coreRoot.file("encore-core/src/brand.rs")).asText.map { text ->
    Regex("""pub const APP_NAME: &str = "([^"]+)";""").find(text)?.groupValues?.get(1)
        ?: throw GradleException("APP_NAME not found in core/encore-core/src/brand.rs")
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(coreLibs) { task ->
            when (task) {
                is BuildRustCore -> task.outputDir
                is CopyPrebuiltCore -> task.outputDir
                else -> error("unexpected task")
            }
        }
        // The generated bindings and token constants, used as they are (never copied).
        variant.sources.kotlin?.apply {
            addStaticSourceDirectory(coreRoot.dir("bindings/kotlin").asFile.absolutePath)
            addStaticSourceDirectory(coreRoot.dir("bindings/kotlin-android").asFile.absolutePath)
            addStaticSourceDirectory(coreRoot.dir("bindings/tokens").asFile.absolutePath)
        }
        variant.sources.assets?.addGeneratedSourceDirectory(licenceAssets, LicenceAssets::outputDir)
        variant.resValues.put(
            variant.makeResValueKey("string", "app_name"),
            appName.map { ResValue(it, "From core/encore-core/src/brand.rs") },
        )
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.material3)
    implementation(libs.activity.compose)
    implementation(libs.core.ktx)
    implementation(libs.jna) { artifact { type = "aar" } }
    implementation(libs.coroutines.android)
    implementation(libs.rustls.platform.verifier)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.session)
    implementation(libs.media3.datasource)

    debugImplementation(libs.compose.ui.tooling)
    debugImplementation(libs.compose.ui.test.manifest)

    testImplementation(libs.junit)

    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.espresso)
}
