plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// Firebase configuration is package-specific. Keep local/debug builds usable while the
// com.mousekeeper.app Firebase client is not registered, and surface UNCONFIGURED in Dart.
if (file("google-services.json").exists()) {
    apply(plugin = "com.google.gms.google-services")
}

val releaseStorePath = System.getenv("MOUSEKEEPER_ANDROID_KEYSTORE_PATH")
val releaseKeyAlias = System.getenv("MOUSEKEEPER_ANDROID_KEY_ALIAS")
val releaseStorePassword = System.getenv("MOUSEKEEPER_ANDROID_STORE_PASSWORD")
val releaseKeyPassword = System.getenv("MOUSEKEEPER_ANDROID_KEY_PASSWORD")
val releaseSigningConfigured = listOf(
    releaseStorePath,
    releaseKeyAlias,
    releaseStorePassword,
    releaseKeyPassword,
).all { !it.isNullOrBlank() }
val releaseRequested = gradle.startParameter.taskNames.any {
    it.contains("release", ignoreCase = true)
}

if (releaseRequested && !releaseSigningConfigured) {
    throw GradleException(
        "UNCONFIGURED: MOUSEKEEPER_ANDROID_KEYSTORE_PATH, " +
            "MOUSEKEEPER_ANDROID_KEY_ALIAS, MOUSEKEEPER_ANDROID_STORE_PASSWORD, " +
            "MOUSEKEEPER_ANDROID_KEY_PASSWORD",
    )
}

android {
    namespace = "com.mousekeeper.app"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "com.mousekeeper.app"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        if (releaseSigningConfigured) {
            create("release") {
                storeFile = file(releaseStorePath!!)
                keyAlias = releaseKeyAlias
                storePassword = releaseStorePassword
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
