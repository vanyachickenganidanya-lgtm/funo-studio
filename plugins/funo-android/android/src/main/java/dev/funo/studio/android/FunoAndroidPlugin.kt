package dev.funo.studio.android

import android.app.Activity
import android.content.Intent
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import net.kdt.pojavlaunch.PojavApplication
import net.kdt.pojavlaunch.TestStorageActivity
import org.json.JSONObject

@TauriPlugin
class FunoAndroidPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun toolchainStatus(invoke: Invoke) {
        val request = invoke.parseArgs(ToolchainRequest::class.java)
        background(invoke) { ToolchainManager.status(activity.applicationContext, request) }
    }

    @Command
    fun installToolchain(invoke: Invoke) {
        val request = invoke.parseArgs(ToolchainRequest::class.java)
        background(invoke) { ToolchainManager.install(activity.applicationContext, request) }
    }

    @Command
    fun buildMinecraft(invoke: Invoke) {
        val request = invoke.parseArgs(BuildRequest::class.java)
        background(invoke) { FunoBuildController.startAndWait(activity.applicationContext, request) }
    }

    @Command
    fun openLauncher(invoke: Invoke) {
        activity.runOnUiThread {
            try {
                val intent = Intent(activity, TestStorageActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                activity.startActivity(intent)
                invoke.resolve(JSObject().apply { put("value", "Встроенный Minecraft Launcher открыт") })
            } catch (error: Throwable) {
                invoke.reject(error.message ?: error.javaClass.simpleName)
            }
        }
    }

    private fun background(invoke: Invoke, block: () -> JSONObject) {
        PojavApplication.sExecutorService.execute {
            try {
                invoke.resolve(block().toJsObject())
            } catch (error: Throwable) {
                invoke.reject(error.message ?: error.javaClass.simpleName)
            }
        }
    }

    private fun JSONObject.toJsObject(): JSObject {
        val result = JSObject()
        val iterator = keys()
        while (iterator.hasNext()) {
            val key = iterator.next()
            result.put(key, get(key))
        }
        return result
    }
}
