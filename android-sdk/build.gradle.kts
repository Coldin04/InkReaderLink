plugins {
    id("com.android.library") version "9.1.1"
    id("com.vanniktech.maven.publish") version "0.37.0"
}

val sdkVersion = providers.gradleProperty("sdkVersion")
    .orElse(providers.environmentVariable("SDK_VERSION"))
    .orElse("0.1.0-preview.1")
    .get()

group = "com.cold04"
version = sdkVersion

android {
    namespace = "com.cold04.picobookmgr"
    compileSdk = 37

    defaultConfig {
        minSdk = 28
        consumerProguardFiles("consumer-rules.pro")
    }

    sourceSets {
        getByName("main") {
            kotlin.directories.add(layout.buildDirectory.dir("generated/sources/uniffi").get().asFile.absolutePath)
            jniLibs.directories.add(layout.buildDirectory.dir("generated/jniLibs").get().asFile.absolutePath)
        }
    }
}

dependencies {
    implementation("net.java.dev.jna:jna:5.18.1@aar")
    api("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4")
}

mavenPublishing {
    coordinates("com.cold04", "picobookmgr", sdkVersion)
    publishToMavenCentral(automaticRelease = true)
    if (providers.gradleProperty("signingInMemoryKey").isPresent) {
        signAllPublications()
    }

    pom {
        name.set("PicoBook Manager SDK for Android")
        description.set("Android bindings and native libraries for the PicoBook device management SDK")
        url.set("https://github.com/Coldin04/PicoBook_SDK")
        licenses {
            license {
                name.set("MIT License")
                url.set("https://opensource.org/licenses/MIT")
                distribution.set("repo")
            }
        }
        developers {
            developer {
                id.set("coldin04")
                name.set("Coldin04")
            }
        }
        scm {
            url.set("https://github.com/Coldin04/PicoBook_SDK")
            connection.set("scm:git:https://github.com/Coldin04/PicoBook_SDK.git")
            developerConnection.set("scm:git:ssh://git@github.com/Coldin04/PicoBook_SDK.git")
        }
    }
}
