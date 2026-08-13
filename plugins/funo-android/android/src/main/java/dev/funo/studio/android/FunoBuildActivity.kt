package dev.funo.studio.android

import android.os.Bundle
import android.view.Gravity
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import com.google.gson.Gson
import com.oracle.dalvik.VMLauncher
import net.kdt.pojavlaunch.Tools
import net.kdt.pojavlaunch.multirt.MultiRTUtils
import net.kdt.pojavlaunch.utils.JREUtils
import org.json.JSONObject
import java.io.File

class FunoBuildActivity : AppCompatActivity() {
    companion object {
        const val EXTRA_REQUEST_PATH = "funo.build.REQUEST_PATH"
        const val EXTRA_STATUS_PATH = "funo.build.STATUS_PATH"
    }

    private lateinit var message: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        message = TextView(this).apply {
            text = "Funo Studio готовит локальную Gradle-сборку…"
            textSize = 17f
            gravity = Gravity.CENTER
            setPadding(32, 32, 32, 32)
        }
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            addView(ProgressBar(this@FunoBuildActivity))
            addView(message)
        })

        val requestPath = intent.getStringExtra(EXTRA_REQUEST_PATH)
        val statusPath = intent.getStringExtra(EXTRA_STATUS_PATH)
        if (requestPath == null || statusPath == null) {
            finish()
            return
        }
        Thread { runBuild(File(requestPath), File(statusPath)) }.start()
    }

    private fun runBuild(requestFile: File, statusFile: File) {
        val started = System.currentTimeMillis()
        try {
            val request = Gson().fromJson(requestFile.readText(), BuildRequest::class.java)
            val project = File(request.projectRoot).canonicalFile
            val runtimeHome = ToolchainManager.runtimeHome(this, request.minecraftVersion)
            val gradleHome = ToolchainManager.gradleHome(this, request.loader, request.minecraftVersion)
                ?: throw IllegalStateException("Gradle исчез после проверки")
            val runtimeName = runtimeHome.name
            val runtime = MultiRTUtils.read(runtimeName)
            if (runtime.versionString == null) throw IllegalStateException("Не удалось прочитать release установленного JDK")

            val initScript = File(statusFile.parentFile, "funo-build-init.gradle")
            initScript.writeText(buildFinishedScript(statusFile, started))
            FunoBuildController.writeStatus(statusFile, "running", false, "Gradle собирает мод локально…", null, 0)
            runOnUiThread { message.text = "Gradle собирает Minecraft-мод. Не закрывайте Funo Studio." }

            JREUtils.relocateLibPath(runtime, runtimeHome.absolutePath)
            JREUtils.setJavaEnvironment(this, runtimeHome.absolutePath)
            JREUtils.initJavaRuntime(runtimeHome.absolutePath)
            JREUtils.setupExitMethod(application)
            JREUtils.initializeHooks()
            JREUtils.chdir(project.absolutePath)

            val launcher = File(gradleHome, "lib/gradle-launcher-${gradleHome.name.removePrefix("gradle-")}.jar")
            if (!launcher.isFile) throw IllegalStateException("Не найден gradle-launcher.jar")
            val args = mutableListOf(
                "java",
                "-Xmx1024m",
                "-XX:MaxMetaspaceSize=384m",
                "-XX:+HeapDumpOnOutOfMemoryError",
                "-Dfile.encoding=UTF-8",
                "-Duser.country=",
                "-Duser.language=en",
                "-Duser.variant=",
                "-Djava.home=${runtimeHome.absolutePath}",
                "-Duser.home=${File(gradleHome.parentFile.parentFile, "home").absolutePath}",
                "-Dgradle.user.home=${File(gradleHome.parentFile.parentFile, "cache").absolutePath}"
            )
            File(gradleHome, "lib/agents").listFiles()
                ?.firstOrNull { it.name.startsWith("gradle-instrumentation-agent-") && it.extension == "jar" }
                ?.let { args += "-javaagent:${it.absolutePath}" }
            args += listOf(
                "-classpath", launcher.absolutePath,
                "org.gradle.launcher.GradleMain",
                "build",
                "--no-daemon",
                "--console=plain",
                "--stacktrace",
                "--init-script", initScript.absolutePath
            )

            val exitCode = VMLauncher.launchJVM(args.toTypedArray())
            val existing = try { JSONObject(statusFile.readText()) } catch (_: Throwable) { JSONObject() }
            if (existing.optString("state") !in listOf("done", "failed")) {
                FunoBuildController.writeStatus(
                    statusFile,
                    if (exitCode == 0) "done" else "failed",
                    exitCode == 0,
                    if (exitCode == 0) "Gradle завершил сборку" else "Gradle завершился с кодом $exitCode",
                    findArtifact(project)?.absolutePath,
                    System.currentTimeMillis() - started
                )
            }
        } catch (error: Throwable) {
            FunoBuildController.writeStatus(
                statusFile,
                "failed",
                false,
                error.message ?: error.javaClass.simpleName,
                null,
                System.currentTimeMillis() - started,
                error.stackTraceToString()
            )
        } finally {
            runOnUiThread { finishAndRemoveTask() }
        }
    }

    private fun buildFinishedScript(statusFile: File, started: Long): String {
        val status = groovyQuote(statusFile.absolutePath)
        return """
            import groovy.json.JsonOutput
            gradle.buildFinished { result ->
                def jars = gradle.rootProject.fileTree(dir: new File(gradle.rootProject.projectDir, 'build/libs'), include: ['*.jar'])
                    .files.findAll { !it.name.endsWith('-sources.jar') && !it.name.endsWith('-dev.jar') }
                def artifact = jars ? jars.max { it.lastModified() }.absolutePath : null
                def failure = result.failure
                def value = [
                    state: failure == null ? 'done' : 'failed',
                    success: failure == null,
                    message: failure == null ? 'Minecraft-мод собран локально' : (failure.message ?: failure.toString()),
                    artifact: artifact,
                    elapsed_ms: System.currentTimeMillis() - ${started}L,
                    log: failure == null ? '' : failure.toString()
                ]
                def target = new File('${status}')
                def temporary = new File(target.parentFile, target.name + '.gradle.tmp')
                temporary.text = JsonOutput.toJson(value)
                if (!temporary.renameTo(target)) target.text = JsonOutput.toJson(value)
            }
        """.trimIndent()
    }

    private fun groovyQuote(value: String) = value.replace("\\", "\\\\").replace("'", "\\'")

    private fun findArtifact(project: File): File? = File(project, "build/libs").listFiles()
        ?.filter { it.isFile && it.extension == "jar" && !it.name.endsWith("-sources.jar") && !it.name.endsWith("-dev.jar") }
        ?.maxByOrNull { it.lastModified() }
}
