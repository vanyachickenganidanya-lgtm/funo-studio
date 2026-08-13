package dev.funo.studio.android

import android.content.Context
import android.os.Build
import android.os.StatFs
import net.kdt.pojavlaunch.Tools
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.DigestInputStream
import java.security.MessageDigest
import java.util.Locale
import java.util.zip.ZipInputStream

internal const val FREE_SPACE_RESERVE = 30L * 1024L * 1024L * 1024L
private const val JDK_INSTALL_ESTIMATE = 900L * 1024L * 1024L
private const val GRADLE_INSTALL_ESTIMATE = 550L * 1024L * 1024L
private const val GITHUB_REPOSITORY = "feeldev12/nerion-android-jdk-build"

internal data class ToolchainRequest(
    val projectRoot: String = "",
    val minecraftVersion: String = "1.21.1",
    val loader: String = "fabric",
    val checkUpdates: Boolean = false,
    val destinationRoot: String = ""
)

internal data class BuildRequest(
    val projectRoot: String = "",
    val source: String = "",
    val minecraftVersion: String = "1.21.1",
    val loader: String = "fabric"
)

private data class Artifact(
    val id: Long,
    val name: String,
    val size: Long,
    val digest: String,
    val downloadUrl: String
)

internal object ToolchainManager {
    fun status(context: Context, request: ToolchainRequest): JSONObject {
        ensurePojavConstants(context)
        val requiredJava = javaForMinecraft(request.minecraftVersion)
        val runtimeJava = runtimeJava(requiredJava)
        val gradleVersion = recommendedGradle(request.loader, request.minecraftVersion, requiredJava)
        val runtimeHome = File(Tools.MULTIRT_HOME, "funo-jdk-$runtimeJava")
        val gradleHome = installedGradleHome(context, gradleVersion)
        val jdkReady = isFullJdk(runtimeHome, runtimeJava)
        val gradleReady = gradleHome != null
        val estimate = (if (jdkReady) 0 else JDK_INSTALL_ESTIMATE) +
            (if (gradleReady) 0 else GRADLE_INSTALL_ESTIMATE)
        val volumes = storageVolumes(context, estimate)
        val recommendedRoot = volumes.firstOrNull { it.optBoolean("eligible") }
            ?.optString("install_root")
            ?: volumes.firstOrNull()?.optString("install_root").orEmpty()
        val latestAvailable = if (request.checkUpdates) {
            try { findArtifact(runtimeJava) != null } catch (_: Throwable) { false }
        } else true

        val jdk = JSONObject()
            .put("found", runtimeHome.isDirectory)
            .put("compatible", jdkReady)
            .put("managed", runtimeHome.isDirectory)
            .put("version", if (jdkReady) runtimeJava.toString() else "")
            .put("latest_version", runtimeJava.toString())
            .put("path", if (runtimeHome.isDirectory) runtimeHome.absolutePath else "")
            .put("detail", when {
                jdkReady && requiredJava == 16 -> "Android JDK 17 готов; Java 16 bytecode создаётся через --release 16"
                jdkReady -> "Полный Android JDK $runtimeJava готов (javac и модули компилятора найдены)"
                runtimeHome.isDirectory -> "Runtime повреждён или не содержит javac/jdk.compiler"
                request.checkUpdates && !latestAvailable -> "Для ABI ${artifactAbi()} сейчас нет полного JDK $runtimeJava"
                else -> "Нужен полный Android JDK $runtimeJava"
            })
            .put("update_available", false)

        val gradle = JSONObject()
            .put("found", gradleReady)
            .put("compatible", gradleReady)
            .put("managed", gradleReady)
            .put("version", if (gradleReady) gradleVersion else "")
            .put("latest_version", gradleVersion)
            .put("path", gradleHome?.absolutePath.orEmpty())
            .put("detail", if (gradleReady) "Gradle $gradleVersion готов" else "Нужен Gradle $gradleVersion")
            .put("update_available", false)

        val ready = jdkReady && gradleReady
        return JSONObject()
            .put("required_java", requiredJava)
            .put("recommended_gradle", gradleVersion)
            .put("reserve_bytes", FREE_SPACE_RESERVE)
            .put("estimated_install_bytes", estimate)
            .put("jdk", jdk)
            .put("gradle", gradle)
            .put("volumes", JSONArray(volumes))
            .put("recommended_install_root", recommendedRoot)
            .put("ready", ready)
            .put("has_updates", false)
            .put("message", when {
                ready -> "Портативный Android JDK и Gradle готовы. Сборка выполняется локально."
                volumes.none { it.optBoolean("eligible") } -> "Для установки нужно сохранить резерв 30 ГиБ. Освободите место или выберите другой доступный том."
                request.checkUpdates && !latestAvailable -> "Не найден актуальный ABI-совместимый полный JDK $runtimeJava."
                else -> "Установите портативную Java и Gradle, чтобы собирать моды на устройстве."
            })
    }

    fun install(context: Context, request: ToolchainRequest): JSONObject {
        ensurePojavConstants(context)
        val before = status(context, request)
        if (before.optBoolean("ready")) return before

        val requestedRoot = request.destinationRoot.ifBlank {
            before.optString("recommended_install_root")
        }
        val allowedVolume = storageVolumes(context, before.optLong("estimated_install_bytes"))
            .firstOrNull { it.optString("install_root") == requestedRoot }
            ?: throw IllegalArgumentException("Выберите папку установки из списка доступных томов Studio")
        if (!allowedVolume.optBoolean("eligible")) {
            throw IllegalStateException("После установки на выбранном томе должно остаться не менее 30 ГиБ")
        }

        val requiredJava = javaForMinecraft(request.minecraftVersion)
        val runtimeJava = runtimeJava(requiredJava)
        val runtimeHome = File(Tools.MULTIRT_HOME, "funo-jdk-$runtimeJava")
        if (!isFullJdk(runtimeHome, runtimeJava)) installJdk(context, runtimeJava)

        val gradleVersion = recommendedGradle(request.loader, request.minecraftVersion, requiredJava)
        if (installedGradleHome(context, gradleVersion) == null) {
            installGradle(context, gradleVersion, File(requestedRoot))
        }

        val after = status(context, request.copy(checkUpdates = false))
        if (!after.optBoolean("ready")) {
            throw IllegalStateException("Установка закончилась, но javac, jdk.compiler или Gradle не прошли проверку")
        }
        return after
    }

    fun runtimeHome(context: Context, minecraftVersion: String): File {
        ensurePojavConstants(context)
        return File(Tools.MULTIRT_HOME, "funo-jdk-${runtimeJava(javaForMinecraft(minecraftVersion))}")
    }

    fun gradleHome(context: Context, loader: String, minecraftVersion: String): File? {
        val required = javaForMinecraft(minecraftVersion)
        return installedGradleHome(context, recommendedGradle(loader, minecraftVersion, required))
    }

    private fun installJdk(context: Context, major: Int) {
        val internalAvailable = StatFs(context.filesDir.absolutePath).availableBytes
        if (internalAvailable < JDK_INSTALL_ESTIMATE + 2L * 1024L * 1024L * 1024L) {
            throw IllegalStateException("Во внутреннем хранилище приложения недостаточно места для безопасной установки JVM")
        }
        val artifact = findArtifact(major)
            ?: throw IllegalStateException("Полный Android JDK $major для ABI ${artifactAbi()} сейчас не опубликован")
        val work = File(context.cacheDir, "funo-jdk-install").apply {
            deleteRecursively()
            mkdirs()
        }
        val artifactZip = File(work, "artifact.zip")
        download(artifact.downloadUrl, artifactZip, artifact.digest.removePrefix("sha256:"))

        var tarXz: File? = null
        var checksumFile: File? = null
        ZipInputStream(BufferedInputStream(FileInputStream(artifactZip))).use { zip ->
            while (true) {
                val entry = zip.nextEntry ?: break
                val name = File(entry.name).name
                if (entry.isDirectory) continue
                if (!name.endsWith(".tar.xz") && !name.endsWith(".tar.xz.sha256")) continue
                val output = File(work, name)
                BufferedOutputStream(FileOutputStream(output)).use { zip.copyTo(it) }
                if (name.endsWith(".sha256")) checksumFile = output else tarXz = output
            }
        }
        val archive = tarXz ?: throw IllegalStateException("В Actions artifact нет архива JDK tar.xz")
        val expectedInner = checksumFile?.readText()?.trim()?.split(Regex("\\s+"))?.firstOrNull()
            ?: throw IllegalStateException("В Actions artifact нет контрольной суммы JDK")
        verifySha256(archive, expectedInner)

        val destination = File(Tools.MULTIRT_HOME, "funo-jdk-$major")
        val staging = File(Tools.MULTIRT_HOME, ".funo-jdk-$major.tmp")
        staging.deleteRecursively()
        staging.mkdirs()
        try {
            SecureArchives.extractTarXz(archive, staging)
            if (!isFullJdk(staging, major)) {
                throw IllegalStateException("Архив не содержит полный JDK $major (javac/jdk.compiler)")
            }
            destination.deleteRecursively()
            if (!staging.renameTo(destination)) {
                staging.copyRecursively(destination, overwrite = true)
                staging.deleteRecursively()
            }
        } catch (error: Throwable) {
            staging.deleteRecursively()
            throw error
        } finally {
            work.deleteRecursively()
        }
    }

    private fun installGradle(context: Context, version: String, selectedRoot: File) {
        val checksum = gradleChecksum(version)
        val root = File(selectedRoot, "gradle").apply { mkdirs() }
        val destination = File(root, "gradle-$version")
        if (File(destination, "lib/gradle-launcher-$version.jar").isFile) {
            saveGradleHome(context, version, destination)
            return
        }
        val zip = File(context.cacheDir, "gradle-$version-bin.zip")
        download("https://downloads.gradle.org/distributions/gradle-$version-bin.zip", zip, checksum)
        val staging = File(root, ".gradle-$version.tmp").apply {
            deleteRecursively()
            mkdirs()
        }
        try {
            SecureArchives.extractZip(zip, staging)
            val unpacked = File(staging, "gradle-$version")
            if (!File(unpacked, "lib/gradle-launcher-$version.jar").isFile) {
                throw IllegalStateException("Архив Gradle $version имеет неожиданный формат")
            }
            destination.deleteRecursively()
            if (!unpacked.renameTo(destination)) {
                unpacked.copyRecursively(destination, overwrite = true)
            }
            saveGradleHome(context, version, destination)
        } finally {
            zip.delete()
            staging.deleteRecursively()
        }
    }

    private fun findArtifact(major: Int): Artifact? {
        val name = "nerion-jdk$major-${artifactAbi()}"
        val endpoint = "https://api.github.com/repos/$GITHUB_REPOSITORY/actions/artifacts?per_page=100&name=$name"
        val json = JSONObject(readText(endpoint))
        val artifacts = json.optJSONArray("artifacts") ?: return null
        var selected: JSONObject? = null
        for (index in 0 until artifacts.length()) {
            val candidate = artifacts.getJSONObject(index)
            if (candidate.optString("name") != name || candidate.optBoolean("expired", true)) continue
            if (selected == null || candidate.optLong("id") > selected.optLong("id")) selected = candidate
        }
        val value = selected ?: return null
        val id = value.getLong("id")
        return Artifact(
            id = id,
            name = name,
            size = value.optLong("size_in_bytes"),
            digest = value.optString("digest"),
            downloadUrl = "https://api.github.com/repos/$GITHUB_REPOSITORY/actions/artifacts/$id/zip"
        )
    }

    private fun readText(url: String): String {
        val connection = openFollowingRedirects(url)
        return try {
            if (connection.responseCode !in 200..299) {
                throw IllegalStateException("GitHub API вернул HTTP ${connection.responseCode}")
            }
            connection.inputStream.bufferedReader().use { it.readText() }
        } finally { connection.disconnect() }
    }

    private fun download(url: String, destination: File, expectedSha256: String) {
        destination.parentFile?.mkdirs()
        val connection = openFollowingRedirects(url)
        try {
            if (connection.responseCode !in 200..299) {
                throw IllegalStateException("Сервер загрузки вернул HTTP ${connection.responseCode}")
            }
            val digest = MessageDigest.getInstance("SHA-256")
            DigestInputStream(BufferedInputStream(connection.inputStream), digest).use { input ->
                BufferedOutputStream(FileOutputStream(destination)).use { output -> input.copyTo(output) }
            }
            val actual = digest.digest().toHex()
            if (expectedSha256.isNotBlank() && !actual.equals(expectedSha256, ignoreCase = true)) {
                destination.delete()
                throw SecurityException("SHA-256 загрузки не совпадает: ожидался $expectedSha256, получен $actual")
            }
        } finally { connection.disconnect() }
    }

    private fun openFollowingRedirects(initialUrl: String): HttpURLConnection {
        var current = initialUrl
        repeat(8) {
            val connection = (URL(current).openConnection() as HttpURLConnection).apply {
                instanceFollowRedirects = false
                connectTimeout = 30_000
                readTimeout = 120_000
                setRequestProperty("Accept", "application/vnd.github+json")
                setRequestProperty("X-GitHub-Api-Version", "2022-11-28")
                setRequestProperty("User-Agent", "Funo-Studio-Android")
            }
            val code = connection.responseCode
            if (code !in 300..399) return connection
            val next = connection.getHeaderField("Location")
                ?: throw IllegalStateException("Сервер вернул перенаправление без Location")
            connection.disconnect()
            current = URL(URL(current), next).toString()
        }
        throw IllegalStateException("Слишком много перенаправлений при загрузке")
    }

    private fun verifySha256(file: File, expected: String) {
        val digest = MessageDigest.getInstance("SHA-256")
        FileInputStream(file).use { input ->
            val buffer = ByteArray(128 * 1024)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        val actual = digest.digest().toHex()
        if (!actual.equals(expected, ignoreCase = true)) {
            throw SecurityException("Внутренняя SHA-256 JDK не совпадает")
        }
    }

    private fun storageVolumes(context: Context, estimate: Long): List<JSONObject> {
        val seen = mutableSetOf<String>()
        val internalAvailable = StatFs(context.filesDir.absolutePath).availableBytes
        return context.getExternalFilesDirs(null).mapIndexedNotNull { index, external ->
            if (external == null) return@mapIndexedNotNull null
            val installRoot = File(external, "funo-toolchains")
            val canonical = try { installRoot.canonicalPath } catch (_: Throwable) { installRoot.absolutePath }
            if (!seen.add(canonical)) return@mapIndexedNotNull null
            val stat = try { StatFs(external.absolutePath) } catch (_: Throwable) { return@mapIndexedNotNull null }
            val free = stat.availableBytes
            val total = stat.totalBytes
            val after = (free - estimate).coerceAtLeast(0)
            val jdkInternalOk = internalAvailable >= (if (isAnyManagedJdkNeeded()) 2L * 1024L * 1024L * 1024L else 0L)
            JSONObject()
                .put("id", if (index == 0) "Память устройства" else "Дополнительный том ${index + 1}")
                .put("root", external.absolutePath)
                .put("install_root", canonical)
                .put("free_bytes", free)
                .put("total_bytes", total)
                .put("available_after_bytes", after)
                .put("eligible", after >= FREE_SPACE_RESERVE && jdkInternalOk)
                .put("current", index == 0)
        }
    }

    private fun isAnyManagedJdkNeeded() = true

    private fun ensurePojavConstants(context: Context) {
        if (Tools.MULTIRT_HOME == null) Tools.initEarlyConstants(context.applicationContext)
        File(Tools.MULTIRT_HOME).mkdirs()
    }

    private fun isFullJdk(home: File, major: Int): Boolean {
        if (!File(home, "release").isFile || !File(home, "bin/javac").exists()) return false
        return if (major <= 8) {
            File(home, "lib/tools.jar").isFile || File(home, "jre/lib/rt.jar").isFile
        } else {
            File(home, "jmods/jdk.compiler.jmod").isFile || File(home, "lib/modules").isFile
        }
    }

    private fun installedGradleHome(context: Context, version: String): File? {
        val value = context.getSharedPreferences("funo_android_tools", Context.MODE_PRIVATE)
            .getString("gradle_home_$version", null) ?: return null
        val home = File(value)
        return home.takeIf { File(it, "lib/gradle-launcher-$version.jar").isFile }
    }

    private fun saveGradleHome(context: Context, version: String, home: File) {
        context.getSharedPreferences("funo_android_tools", Context.MODE_PRIVATE)
            .edit().putString("gradle_home_$version", home.absolutePath).apply()
    }

    private fun runtimeJava(required: Int): Int = if (required == 16) 17 else required

    internal fun javaForMinecraft(version: String): Int {
        val parts = version.split('.').mapNotNull { it.toIntOrNull() }
        val major = parts.getOrElse(0) { 1 }
        val minor = parts.getOrElse(1) { 0 }
        val patch = parts.getOrElse(2) { 0 }
        return when {
            major >= 26 -> 25
            minor > 20 || (minor == 20 && patch >= 5) -> 21
            minor >= 18 -> 17
            minor == 17 -> 16
            else -> 8
        }
    }

    internal fun recommendedGradle(loader: String, version: String, java: Int): String {
        if (loader.lowercase(Locale.US) == "forge") {
            if (java <= 8 && compareMinecraft(version, 1, 16, 0) < 0) return "4.10.3"
            if (java <= 8) return "6.9.4"
            if (java <= 16) return "7.3.3"
            if (java <= 17) return "8.8"
            if (java <= 21) return "8.14.3"
            return "9.4.0"
        }
        if (loader.lowercase(Locale.US) == "neoforge") {
            if (java <= 17) return "8.8"
            if (java <= 21) return "8.14.3"
            return "9.4.0"
        }
        return if (java <= 21) "8.14.3" else "9.4.0"
    }

    private fun compareMinecraft(value: String, major: Int, minor: Int, patch: Int): Int {
        val parts = value.split('.').mapNotNull { it.toIntOrNull() }
        val actual = listOf(parts.getOrElse(0) { 0 }, parts.getOrElse(1) { 0 }, parts.getOrElse(2) { 0 })
        val expected = listOf(major, minor, patch)
        for (index in actual.indices) if (actual[index] != expected[index]) return actual[index].compareTo(expected[index])
        return 0
    }

    private fun artifactAbi(): String {
        val abi = Build.SUPPORTED_ABIS.firstOrNull().orEmpty()
        return when (abi) {
            "arm64-v8a" -> "aarch64"
            "armeabi-v7a" -> "aarch32"
            "x86_64" -> "x86_64"
            "x86" -> "x86"
            else -> throw IllegalStateException("ABI $abi не поддерживается Android JDK")
        }
    }

    private fun gradleChecksum(version: String): String = when (version) {
        "4.10.3" -> "8626cbf206b4e201ade7b87779090690447054bc93f052954c78480fa6ed186e"
        "6.9.4" -> "3e240228538de9f18772a574e99a0ba959e83d6ef351014381acd9631781389a"
        "7.3.3" -> "b586e04868a22fd817c8971330fec37e298f3242eb85c374181b12d637f80302"
        "8.8" -> "a4b4158601f8636cdeeab09bd76afb640030bb5b144aafe261a5e8af027dc612"
        "8.14.3" -> "bd71102213493060956ec229d946beee57158dbd89d0e62b91bca0fa2c5f3531"
        "9.4.0" -> "60ea723356d81263e8002fec0fcf9e2b0eee0c0850c7a3d7ab0a63f2ccc601f3"
        else -> throw IllegalArgumentException("Для Gradle $version не закреплена SHA-256")
    }

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }
}
