// 冻住**宿主侧**的随机源。
//
// `determinism.js` 冻的是 JS 引擎那一面(`Date` / `Math.random`);这里冻的是
// Java 那一面 —— `JsExtensions.randomUUID()` 是 `UUID.randomUUID()`,底下取
// `SecureRandom`。实测两个语料用例(`js-corpus-2033d8071d` / `d3c3112495`)
// 把 UUID 拼进签名串,裁判**每次跑结果都不同** —— 那不是实现差异,是差分自身的
// 噪声,永远不可能绿。契约见 fixtures/cases/js-host/README.md「不可差分面」:
// 宿主侧的非确定性一律取固定值。
//
// 手法:插一个优先级最高的 JCA Provider,让 `new SecureRandom()` 拿到定死的
// 字节流。字节流的定义必须**被测侧能逐字复现**,故取最朴素的一种:
//
//     block(i) = SHA-256( "rubato-difftest" || big-endian u64(i) )
//
// i 从 0 开始,按需拼接取用。被测侧 `js_case_runner` 用同一条定义。
package jsharness

import java.security.MessageDigest
import java.security.Provider
import java.security.SecureRandom
import java.security.SecureRandomSpi
import java.security.Security

object DeterministicRandom {
    const val SEED = "rubato-difftest"

    private var installed = false

    /** 幂等;必须在第一次用到 `UUID.randomUUID()` **之前**装(它会缓存实例) */
    @Synchronized
    fun install() {
        if (installed) return
        installed = true
        Security.insertProviderAt(DeterministicProvider(), 1)
    }

    /** SHA-256 计数器流。被测侧 `deterministic_random_bytes` 逐字对齐。 */
    fun bytesAt(counter: Long, into: ByteArray, offset: Int): Int {
        val md = MessageDigest.getInstance("SHA-256")
        md.update(SEED.toByteArray(Charsets.UTF_8))
        val ctr = ByteArray(8)
        for (i in 0 until 8) ctr[i] = (counter ushr (56 - 8 * i)).toByte()
        md.update(ctr)
        val block = md.digest()
        val n = minOf(block.size, into.size - offset)
        System.arraycopy(block, 0, into, offset, n)
        return n
    }

    private class DeterministicProvider : Provider(
        "RubatoDeterministic", "1.0", "差分用:定死的 SecureRandom(见 DeterministicRandom)"
    ) {
        init {
            put("SecureRandom.NativePRNG", Spi::class.java.name)
            put("SecureRandom.SHA1PRNG", Spi::class.java.name)
            put("SecureRandom.DRBG", Spi::class.java.name)
        }
    }

    /**
     * 计数器是**全进程一份**、按 case 复位的。
     *
     * 为什么不能按实例来:`UUID.randomUUID()` 用的是 `UUID.Holder.numberGenerator`
     * —— 一个 **JVM 级单例**,建一次就不再建。计数器挂在实例上就等于挂在整个
     * run 上,第 N 个 case 拿到什么值取决于前面所有 case 消耗了多少 —— 用例之间
     * 串味,被测侧没法对齐。故 [reset] 由 Main 每 case 调一次。
     */
    private val counter = java.util.concurrent.atomic.AtomicLong(0)

    /** 每个 case 开头调:随机流回到 block 0 */
    fun reset() {
        counter.set(0)
    }

    class Spi : SecureRandomSpi() {
        override fun engineSetSeed(seed: ByteArray?) = Unit // 定死,不吃外部种子

        override fun engineNextBytes(bytes: ByteArray) {
            var off = 0
            while (off < bytes.size) {
                off += bytesAt(counter.getAndIncrement(), bytes, off)
            }
        }

        override fun engineGenerateSeed(numBytes: Int): ByteArray =
            ByteArray(numBytes).also { engineNextBytes(it) }
    }
}
