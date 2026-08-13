package dev.funo.studio.android

import android.content.Context
import android.content.Intent
import com.google.gson.Gson
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID

internal object FunoBuildController {
    private const val TIMEOUT_MS = 2L * 60L * 60L * 1000L

    fun startAndWait(context: Context, request: BuildRequest): JSONObject {
        val toolStatus = ToolchainManager.status(
            context,
            ToolchainRequest(
                projectRoot = request.projectRoot,
                minecraftVersion = request.minecraftVersion,
                loader = request.loader
            )
        )
        if (!toolStatus.optBoolean("ready")) {
            throw IllegalStateException("Сначала установите портативную Java и Gradle")
        }
        val project = File(request.projectRoot).canonicalFile
        val privateRoots = listOf(context.dataDir.canonicalFile, context.filesDir.canonicalFile, context.noBackupFilesDir.canonicalFile)
        val isPrivate = privateRoots.any { project.path == it.path || project.path.startsWith(it.path + File.separator) }
        if (!isPrivate || !File(project, "settings.gradle").isFile) {
            throw SecurityException("Android-сборка разрешена только для подготовленного приватного проекта Funo")
        }

        val id = UUID.randomUUID().toString()
        val directory = File(context.noBackupFilesDir, "build-jobs/$id").apply { mkdirs() }
        val requestFile = File(directory, "request.json")
        val statusFile = File(directory, "status.json")
        requestFile.writeText(Gson().toJson(request))
        writeStatus(statusFile, "queued", false, "Сборка ожидает запуска JVM", null, 0)

        val intent = Intent(context, FunoBuildActivity::class.java)
            .putExtra(FunoBuildActivity.EXTRA_REQUEST_PATH, requestFile.absolutePath)
            .putExtra(FunoBuildActivity.EXTRA_STATUS_PATH, statusFile.absolutePath)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)

        val started = System.currentTimeMillis()
        while (System.currentTimeMillis() - started < TIMEOUT_MS) {
            Thread.sleep(400)
            val status = try { JSONObject(statusFile.readText()) } catch (_: Throwable) { continue }
            when (status.optString("state")) {
                "done", "failed" -> return buildResult(status)
            }
        }
        throw IllegalStateException("Android Gradle-сборка не завершилась за 2 часа; журнал сохранён в ${directory.absolutePath}")
    }

    private fun buildResult(status: JSONObject): JSONObject {
        val success = status.optString("state") == "done" && status.optBoolean("success")
        val detail = status.optString("message")
        val log = status.optString("log")
        val artifact = status.optString("artifact").takeIf { it.isNotBlank() && it != "null" }
        return JSONObject()
            .put("success", success)
            .put("stdout", if (success) detail else "")
            .put("stderr", if (success) "" else listOf(detail, log).filter { it.isNotBlank() }.joinToString("\n"))
            .put("generated_java", "")
            .put("elapsed_ms", status.optLong("elapsed_ms"))
            .put("diagnostics", JSONArray())
            .put("artifact", artifact ?: JSONObject.NULL)
    }

    internal fun writeStatus(
        file: File,
        state: String,
        success: Boolean,
        message: String,
        artifact: String?,
        elapsedMs: Long,
        log: String = ""
    ) {
        file.parentFile?.mkdirs()
        val value = JSONObject()
            .put("state", state)
            .put("success", success)
            .put("message", message)
            .put("artifact", artifact ?: JSONObject.NULL)
            .put("elapsed_ms", elapsedMs)
            .put("log", log)
        val temporary = File(file.parentFile, "${file.name}.tmp")
        temporary.writeText(value.toString())
        if (!temporary.renameTo(file)) {
            file.writeText(value.toString())
            temporary.delete()
        }
    }
}
