package dev.funo.studio.android

import android.system.Os
import org.apache.commons.compress.archivers.tar.TarArchiveInputStream
import org.apache.commons.compress.compressors.xz.XZCompressorInputStream
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream
import java.util.zip.ZipInputStream

internal object SecureArchives {
    private const val MAX_ENTRY_BYTES = 2L * 1024L * 1024L * 1024L

    fun extractTarXz(archive: File, destination: File) {
        destination.mkdirs()
        val root = destination.canonicalFile
        TarArchiveInputStream(
            XZCompressorInputStream(BufferedInputStream(FileInputStream(archive)))
        ).use { tar ->
            while (true) {
                val entry = tar.nextTarEntry ?: break
                val output = safeDestination(root, entry.name)
                when {
                    entry.isDirectory -> output.mkdirs()
                    entry.isSymbolicLink -> {
                        output.parentFile?.mkdirs()
                        val linkName = entry.linkName
                        validateLink(root, output, linkName)
                        output.delete()
                        Os.symlink(linkName, output.absolutePath)
                    }
                    entry.isLink -> {
                        output.parentFile?.mkdirs()
                        val source = safeDestination(root, entry.linkName)
                        if (!source.isFile) throw SecurityException("Некорректная hard link в JDK: ${entry.name}")
                        source.copyTo(output, overwrite = true)
                    }
                    entry.isFile -> {
                        if (entry.size < 0 || entry.size > MAX_ENTRY_BYTES) {
                            throw SecurityException("Недопустимый размер файла в JDK: ${entry.name}")
                        }
                        output.parentFile?.mkdirs()
                        writeLimited(tar, output, entry.size)
                        try { Os.chmod(output.absolutePath, entry.mode and 0x1ff) } catch (_: Throwable) { }
                    }
                }
            }
        }
    }

    fun extractZip(archive: File, destination: File) {
        destination.mkdirs()
        val root = destination.canonicalFile
        ZipInputStream(BufferedInputStream(FileInputStream(archive))).use { zip ->
            while (true) {
                val entry = zip.nextEntry ?: break
                val output = safeDestination(root, entry.name)
                if (entry.isDirectory) {
                    output.mkdirs()
                } else {
                    if (entry.size > MAX_ENTRY_BYTES) throw SecurityException("Недопустимый размер ZIP entry: ${entry.name}")
                    output.parentFile?.mkdirs()
                    writeLimited(zip, output, entry.size)
                }
                zip.closeEntry()
            }
        }
    }

    private fun safeDestination(root: File, name: String): File {
        val clean = name.replace('\\', '/').removePrefix("./")
        if (clean.isBlank() || clean.startsWith('/') || clean.contains('\u0000')) {
            throw SecurityException("Недопустимый путь в архиве")
        }
        val output = File(root, clean).canonicalFile
        val prefix = root.path + File.separator
        if (output.path != root.path && !output.path.startsWith(prefix)) {
            throw SecurityException("Архив пытается записать файл за пределами папки: $name")
        }
        return output
    }

    private fun validateLink(root: File, output: File, linkName: String) {
        if (linkName.isBlank() || linkName.startsWith('/') || linkName.contains('\u0000')) {
            throw SecurityException("Недопустимая символическая ссылка в JDK")
        }
        val resolved = File(output.parentFile, linkName).canonicalFile
        if (!resolved.path.startsWith(root.path + File.separator)) {
            throw SecurityException("Символическая ссылка JDK выходит за пределы runtime")
        }
    }

    private fun writeLimited(input: InputStream, output: File, declaredSize: Long) {
        var total = 0L
        val buffer = ByteArray(128 * 1024)
        BufferedOutputStream(FileOutputStream(output)).use { stream ->
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                total += count
                if (total > MAX_ENTRY_BYTES || (declaredSize >= 0 && total > declaredSize)) {
                    throw SecurityException("Распаковываемый файл превышает допустимый размер")
                }
                stream.write(buffer, 0, count)
                if (declaredSize >= 0 && total == declaredSize) break
            }
        }
    }
}
