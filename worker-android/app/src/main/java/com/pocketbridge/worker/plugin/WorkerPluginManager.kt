package com.pocketbridge.worker.plugin

import android.content.Context
import com.google.gson.Gson
import java.io.File

/**
 * Metadata manifest for a downloaded plugin on Android.
 */
data class MobilePluginManifest(
    val id: String = "",
    val name: String = "",
    val version: String = "",
    val description: String = "",
    val kind: String = "Native",
    val adapter_id: String = ""
)

/**
 * Manages mobile plugin directory, manifest discovery, and execution gating.
 */
class WorkerPluginManager(private val context: Context) {
    private val gson = Gson()

    /**
     * Local storage directory for downloaded plugins:
     * Priority: App private storage, fallback to app external files dir.
     */
    fun getPluginsDir(): File {
        val dir = File(context.filesDir, "plugins")
        if (!dir.exists()) {
            dir.mkdirs()
        }
        return dir
    }

    /**
     * Check if a plugin adapter is installed and ready.
     * `chunk_transform.v1` is always true as the built-in baseline hardware benchmark.
     */
    fun isPluginInstalled(adapterId: String): Boolean {
        if (adapterId == "chunk_transform.v1") return true

        val dir = getPluginsDir()
        val subdirs = dir.listFiles { f -> f.isDirectory } ?: return false
        for (sub in subdirs) {
            val manifestFile = File(sub, "manifest.json")
            if (manifestFile.exists()) {
                try {
                    val manifest = gson.fromJson(manifestFile.readText(), MobilePluginManifest::class.java)
                    if (manifest.id == adapterId || manifest.adapter_id == adapterId) {
                        return true
                    }
                } catch (_: Exception) {}
            }
        }
        return false
    }

    /**
     * List all installed plugin manifests found on the phone.
     */
    fun listInstalledPlugins(): List<MobilePluginManifest> {
        val list = mutableListOf<MobilePluginManifest>()
        val dir = getPluginsDir()
        val subdirs = dir.listFiles { f -> f.isDirectory } ?: return list
        for (sub in subdirs) {
            val manifestFile = File(sub, "manifest.json")
            if (manifestFile.exists()) {
                try {
                    val manifest = gson.fromJson(manifestFile.readText(), MobilePluginManifest::class.java)
                    list.add(manifest)
                } catch (_: Exception) {}
            }
        }
        return list
    }
}
