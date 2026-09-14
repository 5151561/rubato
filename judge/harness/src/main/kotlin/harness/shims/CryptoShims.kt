@file:Suppress("unused")

package io.legado.app.help.crypto

fun ByteArray.toHexString(): String = joinToString("") { "%02x".format(it) }
