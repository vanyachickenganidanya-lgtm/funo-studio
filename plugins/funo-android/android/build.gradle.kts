plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

val amethystRoot = project.file("../../../vendor/amethyst")
val amethystApp = amethystRoot.resolve("app_pojavlauncher")
val buildAmethystComponents by tasks.registering(GradleBuild::class) {
    dir = amethystRoot
    tasks = listOf(
        ":forge_installer:jar",
        ":arc_dns_injector:jar",
        ":methods_injector_agent:jar",
        ":jre_lwjgl3glfw:jar"
    )
}
tasks.matching { it.name == "preBuild" }.configureEach {
    dependsOn(buildAmethystComponents)
}

android {
    namespace = "net.kdt.pojavlaunch"
    compileSdk = 37
    ndkVersion = "27.3.13750724"

    defaultConfig {
        minSdk = 24
        multiDexEnabled = true
        consumerProguardFiles("consumer-rules.pro")
        buildConfigField("String", "VERSION_NAME", "\"Funo-Amethyst-4cf805a\"")
        externalNativeBuild {
            ndkBuild {
                arguments += "APP_SHORT_COMMANDS=true"
            }
        }
    }

    sourceSets {
        getByName("main") {
            java.srcDir(amethystApp.resolve("src/main/java"))
            res.srcDir(amethystApp.resolve("src/main/res"))
            assets.srcDir(amethystApp.resolve("src/main/assets"))
            jniLibs.srcDir(amethystApp.resolve("src/main/jniLibs"))
        }
    }

    externalNativeBuild {
        ndkBuild {
            path = amethystApp.resolve("src/main/jni/Android.mk")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        prefab = true
        buildConfig = true
    }
    packaging {
        jniLibs {
            useLegacyPackaging = true
            keepDebugSymbols += "**/libcutils.so"
            pickFirsts += "**/libbytehook.so"
        }
        resources {
            pickFirsts += "META-INF/*"
        }
    }
    lint {
        abortOnError = false
    }
}

dependencies {
    implementation(project(":tauri-android"))

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.preference:preference:1.2.1")
    implementation("androidx.drawerlayout:drawerlayout:1.2.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0")
    implementation("androidx.annotation:annotation:1.9.1")
    implementation("androidx.constraintlayout:constraintlayout:2.2.0")

    implementation("javax.annotation:javax.annotation-api:1.3.2")
    implementation("commons-codec:commons-codec:1.15")
    implementation("org.apache.commons:commons-compress:1.27.1")
    implementation("org.tukaani:xz:1.10")
    implementation("net.sourceforge.htmlcleaner:htmlcleaner:2.6.1")
    implementation("com.bytedance:bytehook:1.1.2")
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    implementation("com.github.duanhong169:checkerboarddrawable:1.0.2")
    implementation("com.github.PojavLauncherTeam:portrait-sdp:ed33e89cbc")
    implementation("com.github.PojavLauncherTeam:portrait-ssp:6c02fd739b")
    implementation("com.github.Mathias-Boulay:ExtendedView:1.0.0")
    implementation("com.github.Mathias-Boulay:android_gamepad_remapper:2.0.3")
    implementation("com.github.Mathias-Boulay:virtual-joystick-android:1.14")
    implementation("top.fifthlight.touchcontroller:proxy-client-android:0.0.4")

    implementation(fileTree(mapOf("dir" to amethystApp.resolve("libs"), "include" to listOf("*.jar", "*.aar"))))
}

apply(from = amethystRoot.resolve("gradle/prefab_bypass.gradle"))
