// 差分垫片:help/book 下 webBook/实体触到的面。
// 真身 BookHelp.kt/BookExtensions.kt 绑死文件缓存/Uri/LocalBook,挂不进纯 JVM;
// 此处纯函数逐字复刻真身,缓存换成内存实现(差分契约:content 步 needSave=false)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.book

import io.legado.app.constant.AppPattern
import io.legado.app.constant.BookSourceType
import io.legado.app.constant.BookType
import io.legado.app.data.entities.BaseBook
import io.legado.app.data.entities.Book
import io.legado.app.data.entities.BookChapter
import io.legado.app.data.entities.BookSource
import io.legado.app.utils.MD5Utils
import io.legado.app.utils.StringUtils
import org.apache.commons.text.similarity.JaccardSimilarity
import java.util.regex.Pattern
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

// ===== BookExtensions.kt 纯函数子集(逐字复刻) =====

val Book.isAudio: Boolean
    get() = isType(BookType.audio)

val Book.isVideo: Boolean
    get() = isType(BookType.video)

val Book.isImage: Boolean
    get() = isType(BookType.image)

val Book.isLocal: Boolean
    get() {
        if (type == 0) {
            return origin == BookType.localTag || origin.startsWith(BookType.webDavTag)
        }
        return isType(BookType.local)
    }

val Book.isLocalTxt: Boolean
    get() = isLocal && originName.endsWith(".txt", true)

val Book.isEpub: Boolean
    get() = isLocal && originName.endsWith(".epub", true)

val Book.isOnLineTxt: Boolean
    get() = !isLocal && isType(BookType.text)

val Book.isWebFile: Boolean
    get() = isType(BookType.webFile)

val Book.isUpError: Boolean
    get() = isType(BookType.updateError)

val Book.isArchive: Boolean
    get() = isType(BookType.archive)

val Book.isNotShelf: Boolean
    get() = isType(BookType.notShelf)

fun Book.getRemoteUrl(): String? {
    if (origin.startsWith(BookType.webDavTag)) {
        return origin.substring(BookType.webDavTag.length)
    }
    return null
}

fun Book.setType(vararg types: Int) {
    type = 0
    addType(*types)
}

fun Book.addType(vararg types: Int) {
    types.forEach {
        type = type or it
    }
}

fun Book.removeType(vararg types: Int) {
    types.forEach {
        type = type and it.inv()
    }
}

fun Book.removeAllBookType() {
    removeType(BookType.allBookType)
}

fun Book.clearType() {
    type = 0
}

fun Book.isType(bookType: Int): Boolean = type and bookType > 0

fun Book.upType() {
    if (type < 4) {
        type = when (type) {
            BookSourceType.image -> BookType.image
            BookSourceType.audio -> BookType.audio
            BookSourceType.file -> BookType.webFile
            else -> BookType.text
        }
        if (origin == BookType.localTag || origin.startsWith(BookType.webDavTag)) {
            type = type or BookType.local
        }
    }
}

fun Book.primaryStr(): String {
    return origin + bookUrl
}

fun Book.hasVariable(key: String): Boolean {
    return variableMap.contains(key) || io.legado.app.help.RuleBigDataHelp.hasBookVariable(bookUrl, key)
}

fun Book.getFolderNameNoCache(): String {
    return name.replace(AppPattern.fileNameRegex, "").let {
        it.substring(0, min(9, it.length)) + MD5Utils.md5Encode16(bookUrl)
    }
}

fun Book.releaseHtmlData() {
    infoHtml = null
    tocHtml = null
}

fun Book.isSameNameAuthor(other: Any?): Boolean {
    if (other is BaseBook) {
        return name == other.name && author == other.author
    }
    return false
}

fun Book.simulatedTotalChapterNum(): Int {
    // 真身在 readSimulating() 时按日期解锁;差分里 readConfig 恒空 → 直通
    return if (readSimulating()) {
        totalChapterNum
    } else {
        totalChapterNum
    }
}

fun Book.readSimulating(): Boolean {
    return config.readSimulating
}

// ===== BookHelp.kt 被 webBook 触到的面(缓存换内存;格式化逐字复刻) =====

class ContentSaveKey(val bookUrl: String, val chapterIndex: Int)

class ContentSaveToken(
    val key: ContentSaveKey,
    val folderName: String,
    val version: Long,
)

object BookHelp {
    // (bookUrl, index) -> content;差分观察面
    private val contentCache = HashMap<Pair<String, Int>, String>()

    fun formatBookName(name: String): String {
        return name
            .replace(AppPattern.nameRegex, "")
            .trim { it <= ' ' }
    }

    fun formatBookAuthor(author: String): String {
        return author
            .replace(AppPattern.authorRegex, "")
            .trim { it <= ' ' }
    }

    internal fun contentSaveToken(book: Book, bookChapter: BookChapter): ContentSaveToken {
        return ContentSaveToken(
            ContentSaveKey(book.bookUrl, bookChapter.index),
            book.getFolderNameNoCache(),
            0L,
        )
    }

    suspend fun saveContent(
        bookSource: BookSource,
        book: Book,
        bookChapter: BookChapter,
        content: String,
        token: ContentSaveToken = contentSaveToken(book, bookChapter),
        saveChapterMetadata: Boolean = false,
    ): Boolean {
        if (token.key.bookUrl != book.bookUrl || token.key.chapterIndex != bookChapter.index) {
            return false
        }
        if (content.isNotEmpty()) {
            contentCache[book.bookUrl to bookChapter.index] = content
        }
        return true
    }

    fun getContent(book: Book, bookChapter: BookChapter): String? {
        return contentCache[book.bookUrl to bookChapter.index]
    }

    internal fun getContent(
        book: Book,
        bookChapter: BookChapter,
        token: ContentSaveToken,
    ): String? {
        if (token.key.bookUrl != book.bookUrl || token.key.chapterIndex != bookChapter.index) {
            return null
        }
        return contentCache[book.bookUrl to bookChapter.index]
    }

    // BookHelp.kt L?? getDurChapter —— 逐字复刻(章节列表更新后的进度重定位)
    fun getDurChapter(
        oldDurChapterIndex: Int,
        oldDurChapterName: String?,
        newChapterList: List<BookChapter>,
        oldChapterListSize: Int = 0,
        searchAllChapterNumbers: Boolean = false,
    ): Int {
        if (oldDurChapterIndex <= 0) return 0
        if (newChapterList.isEmpty()) return oldDurChapterIndex
        val oldChapterNum = getChapterNum(oldDurChapterName)
        val newChapterSize = newChapterList.size
        val durIndex =
            if (oldChapterListSize == 0) oldDurChapterIndex
            else (oldDurChapterIndex.toLong() * newChapterSize / oldChapterListSize).toInt()
        val min = max(0, min(oldDurChapterIndex, durIndex) - 10)
        val max = min(newChapterSize - 1, max(oldDurChapterIndex, durIndex) + 10)
        findNearestChapterTitleIndex(
            oldDurChapterName,
            newChapterList,
            min..max,
            durIndex,
        )?.let { return it }
        if (searchAllChapterNumbers && oldChapterNum > 0) {
            findNearestChapterNumberIndex(
                newChapterList.map { getChapterNum(it.title) },
                oldChapterNum,
                durIndex,
            )?.let { return it }
        }
        if (oldChapterNum > 0) {
            for (i in min..max) {
                if (getChapterNum(newChapterList[i].title) == oldChapterNum) return i
            }
        }
        return min(max(0, newChapterList.size - 1), oldDurChapterIndex)
    }

    fun getDurChapter(
        oldBook: Book,
        newChapterList: List<BookChapter>
    ): Int {
        return oldBook.run {
            getDurChapter(durChapterIndex, durChapterTitle, newChapterList, totalChapterNum)
        }
    }

    // 真身扫章节缓存目录;harness 无磁盘缓存 → 恒空(ContentProcessor 的
    // removeSameTitleCache 因此永不命中,与被测侧一致)
    fun getChapterFiles(book: Book): HashSet<String> = HashSet()

    // harness 差分观察面
    fun clearCache() = contentCache.clear()
}

// ===== BookHelp.kt 文件级私有辅助(getDurChapter 的依赖)—— 逐字复刻 =====

private val chapterNamePattern1 by lazy {
    Pattern.compile(
        ".*?第([\\d零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+)[章节篇回集话]"
    )
}

@Suppress("RegExpSimplifiable")
private val chapterNamePattern2 by lazy {
    Pattern.compile(
        "^(?:[\\d零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+[,:、])*([\\d零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+)(?:[,:、]|\\.[^\\d])"
    )
}

private val regexA by lazy {
    "\\s".toRegex()
}

private fun getChapterNum(chapterName: String?): Int {
    chapterName ?: return -1
    val chapterName1 = StringUtils.fullToHalf(chapterName).replace(regexA, "")
    return StringUtils.stringToInt(
        (
                chapterNamePattern1.matcher(chapterName1).takeIf { it.find() }
                    ?: chapterNamePattern2.matcher(chapterName1).takeIf { it.find() }
                )?.group(1)
            ?: "-1"
    )
}

private val regexOther by lazy {
    // 所有非字母数字中日韩文字 CJK区+扩展A-F区
    @Suppress("RegExpDuplicateCharacterInClass")
    "[^\\w\\u4E00-\\u9FEF〇\\u3400-\\u4DBF\\u20000-\\u2A6DF\\u2A700-\\u2EBEF]".toRegex()
}

@Suppress("RegExpUnnecessaryNonCapturingGroup", "RegExpSimplifiable")
private val regexB by lazy {
    //章节序号，排除处于结尾的状况，避免将章节名替换为空字串
    "^.*?第(?:[\\d零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+)[章节篇回集话](?!$)|^(?:[\\d零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+[,:、])*(?:[\\d零〇一二两三四五六七八九十百千万壹贰叁肆伍陆柒捌玖拾佰仟]+)(?:[,:、](?!$)|\\.(?=[^\\d]))".toRegex()
}

private val regexC by lazy {
    //前后附加内容，整个章节名都在括号中时只剔除首尾括号，避免将章节名替换为空字串
    "(?!^)(?:[〖【《〔\\[{(][^〖【《〔\\[{()〕》】〗\\]}]+)?[)〕》】〗\\]}]$|^[〖【《〔\\[{(](?:[^〖【《〔\\[{()〕》】〗\\]}]+[〕》】〗\\]})])?(?!$)".toRegex()
}

private fun getPureChapterName(chapterName: String?): String {
    return if (chapterName == null) "" else StringUtils.fullToHalf(chapterName)
        .replace(regexA, "")
        .replace(regexB, "")
        .replace(regexC, "")
        .replace(regexOther, "")
}

private val jaccardSimilarity by lazy {
    JaccardSimilarity()
}

internal fun findNearestChapterTitleIndex(
    oldChapterName: String?,
    newChapterList: List<BookChapter>,
    range: IntRange,
    expectedIndex: Int,
): Int? {
    val oldName = getPureChapterName(oldChapterName)
    if (oldName.isEmpty()) return null
    var bestSimilarity = 0.0
    var bestIndex = 0
    for (i in range) {
        val similarity = jaccardSimilarity.apply(
            oldName,
            getPureChapterName(newChapterList[i].title),
        )
        if (similarity > bestSimilarity ||
            similarity == bestSimilarity && abs(i - expectedIndex) < abs(bestIndex - expectedIndex)
        ) {
            bestSimilarity = similarity
            bestIndex = i
        }
    }
    return bestIndex.takeIf { bestSimilarity > 0.96 }
}

internal fun findNearestChapterNumberIndex(
    chapterNumbers: List<Int>,
    chapterNumber: Int,
    expectedIndex: Int,
): Int? {
    return chapterNumbers.indices
        .filter { chapterNumbers[it] == chapterNumber }
        .minByOrNull { abs(it - expectedIndex) }
}
