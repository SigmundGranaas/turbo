plugins {
    id("turbo.android.application")
    id("turbo.android.hilt")
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.sigmundgranaas.turbo.expressive"

    defaultConfig {
        applicationId = "com.sigmundgranaas.turbo.expressive"
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        vectorDrawables { useSupportLibrary = true }
    }

    // A dedicated, committed keystore so every CI release build is signed
    // identically — sideloaded APKs then reinstall over each other cleanly
    // (no "signatures do not match"). This is a throwaway distribution key,
    // NOT a Play upload key; rotate before any store submission.
    signingConfigs {
        create("sideload") {
            storeFile = file("sideload.keystore")
            storePassword = "turbosideload"
            keyAlias = "turbo-sideload"
            keyPassword = "turbosideload"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName("sideload")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    // ~89% of the APK is MapLibre's native .so. Split release into one APK per
    // ABI so a device downloads only its own ~12 MB slice instead of the ~45 MB
    // universal. ARM only — x86/x86_64 exist solely for emulators, which run debug
    // (a single all-ABI APK; splits stay off there so `app-debug.apk` is unchanged).
    splits {
        abi {
            isEnable = gradle.startParameter.taskNames.any { it.contains("Release") }
            reset()
            include("armeabi-v7a", "arm64-v8a")
            isUniversalApk = false
        }
    }

    buildFeatures { compose = true; buildConfig = true }
}

// Each per-ABI APK needs a distinct versionCode so Play / side-loading treat them
// as one app with correct upgrade ordering (higher = preferred 64-bit ABI). The
// universal/no-split output keeps the base code unchanged.
androidComponents {
    val abiVersionOffsets = mapOf("armeabi-v7a" to 1, "arm64-v8a" to 2)
    onVariants { variant ->
        variant.outputs.forEach { output ->
            val abi = output.filters
                .find { it.filterType == com.android.build.api.variant.FilterConfiguration.FilterType.ABI }
                ?.identifier
            val offset = abiVersionOffsets[abi] ?: return@forEach
            val base = output.versionCode.orNull ?: 1
            output.versionCode.set(base * 10 + offset)
        }
    }
}

dependencies {
    // App is the composition root: design system + feature modules + nav.
    implementation(project(":core:model"))
    implementation(project(":core:data"))
    implementation(project(":core:auth"))
    implementation(project(":core:sync"))
    implementation(project(":core:designsystem"))
    implementation(project(":feature:map"))
    implementation(project(":feature:auth"))
    implementation(project(":feature:settings"))
    implementation(project(":feature:search"))
    implementation(project(":feature:recording"))
    implementation(project(":feature:collections"))

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.activity.compose)

    implementation(libs.androidx.ui)
    implementation(libs.androidx.foundation)
    implementation(libs.androidx.material3)
    debugImplementation(libs.androidx.ui.tooling)

    implementation(libs.androidx.navigation.compose)

    implementation(libs.hilt.navigation.compose)

    testImplementation(libs.junit)
    testImplementation(libs.konsist)
}
