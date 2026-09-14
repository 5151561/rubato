#!/usr/bin/env python3
"""从 judge/engine 的 icu4j 冻结源抽数据表,生成 rust/crates/net/src/icu4j_data.rs。

只抽**数据**(byteMap / ngram / commonChars),识别器逻辑在
net/src/charset_detector.rs 手工移植。改动上游(不会发生,judge 冻结)或
本脚本后需重跑并重跑 fetch 差分。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICU = ROOT / "judge/engine/src/main/java/io/legado/app/lib/icu4j"
OUT = ROOT / "rust/crates/net/src/icu4j_data.rs"

HEX = re.compile(r"0x[0-9a-fA-F]+")


def ints(blob: str):
    return [int(t, 16) for t in HEX.findall(blob)]


def class_chunks(text: str):
    """按 `class CharsetRecog_X` 切块(块延伸到下一个 class 声明)。"""
    marker = re.compile(r"(?:abstract\s+)?static\s+class\s+(CharsetRecog_\w+)")
    hits = list(marker.finditer(text))
    for i, m in enumerate(hits):
        end = hits[i + 1].start() if i + 1 < len(hits) else len(text)
        yield m.group(1), text[m.start():end]


def find_array(chunk: str, name: str):
    # `=` 与 `{` 之间可能夹注释(TODO 行等)
    m = re.search(name + r"\s*=[^{]*\{(.*?)\};", chunk, re.S)
    return ints(m.group(1)) if m else None


def main():
    sbcs = (ICU / "CharsetRecog_sbcs.java").read_text(encoding="utf-8")
    mbcs = (ICU / "CharsetRecog_mbcs.java").read_text(encoding="utf-8")

    out = []
    out.append("//! icu4j CharsetDetector 的数据表——由 tools/gen_icu4j_tables.py")
    out.append("//! 从 judge/engine 冻结源自动抽取,勿手改。")
    out.append("#![allow(clippy::all)]\n")

    def emit_bytemap(rust_name, arr):
        assert len(arr) == 256, (rust_name, len(arr))
        out.append(f"pub static {rust_name}: [u8; 256] = [")
        for i in range(0, 256, 16):
            out.append("    " + ", ".join(f"0x{v:02x}" for v in arr[i:i + 16]) + ",")
        out.append("];\n")

    def emit_ints(rust_name, arr, expect64=True):
        if expect64:
            assert len(arr) == 64, (rust_name, len(arr))
        out.append(f"pub static {rust_name}: [i32; {len(arr)}] = [")
        for i in range(0, len(arr), 8):
            out.append("    " + ", ".join(f"0x{v:08x}" for v in arr[i:i + 8]) + ",")
        out.append("];\n")

    chunks = dict(class_chunks(sbcs))

    # byteMap:8859_2/5/6/7/8/9 定义在 abstract 基类块;8859_1 / windows / koi 在自身块
    for cls, rust in [
        ("CharsetRecog_8859_1", "BYTE_MAP_8859_1"),
        ("CharsetRecog_8859_2", "BYTE_MAP_8859_2"),
        ("CharsetRecog_8859_5", "BYTE_MAP_8859_5"),
        ("CharsetRecog_8859_6", "BYTE_MAP_8859_6"),
        ("CharsetRecog_8859_7", "BYTE_MAP_8859_7"),
        ("CharsetRecog_8859_8", "BYTE_MAP_8859_8"),
        ("CharsetRecog_8859_9", "BYTE_MAP_8859_9"),
        ("CharsetRecog_windows_1251", "BYTE_MAP_WINDOWS_1251"),
        ("CharsetRecog_windows_1256", "BYTE_MAP_WINDOWS_1256"),
        ("CharsetRecog_KOI8_R", "BYTE_MAP_KOI8_R"),
    ]:
        arr = find_array(chunks[cls], r"byteMap")
        assert arr, cls
        emit_bytemap(rust, arr)

    # 多语言 ngram 组(8859-1 / 8859-2)
    for cls, rust in [("CharsetRecog_8859_1", "NGRAMS_8859_1"),
                      ("CharsetRecog_8859_2", "NGRAMS_8859_2")]:
        pairs = re.findall(
            r'new\s+NGramsPlusLang\(\s*"(\w+)",\s*new int\[\]\{(.*?)\}\)',
            chunks[cls], re.S)
        assert pairs, cls
        names = []
        for lang, blob in pairs:
            arr = ints(blob)
            nm = f"{rust}_{lang.upper()}"
            emit_ints(nm, arr)
            names.append((lang, nm))
        out.append(f"pub static {rust}: &[(&str, &[i32; 64])] = &[")
        for lang, nm in names:
            out.append(f'    ("{lang}", &{nm}),')
        out.append("];\n")

    # 单语言 ngram
    for cls, rust in [
        ("CharsetRecog_8859_5_ru", "NGRAMS_8859_5_RU"),
        ("CharsetRecog_8859_6_ar", "NGRAMS_8859_6_AR"),
        ("CharsetRecog_8859_7_el", "NGRAMS_8859_7_EL"),
        ("CharsetRecog_8859_8_I_he", "NGRAMS_8859_8_I_HE"),
        ("CharsetRecog_8859_8_he", "NGRAMS_8859_8_HE"),
        ("CharsetRecog_8859_9_tr", "NGRAMS_8859_9_TR"),
        ("CharsetRecog_windows_1251", "NGRAMS_WINDOWS_1251"),
        ("CharsetRecog_windows_1256", "NGRAMS_WINDOWS_1256"),
        ("CharsetRecog_KOI8_R", "NGRAMS_KOI8_R"),
    ]:
        arr = find_array(chunks[cls], r"ngrams")
        assert arr, cls
        emit_ints(rust, arr)

    # mbcs commonChars
    mchunks = dict(class_chunks(mbcs))
    for cls, rust in [
        ("CharsetRecog_sjis", "COMMON_SJIS"),
        ("CharsetRecog_big5", "COMMON_BIG5"),
        ("CharsetRecog_euc_jp", "COMMON_EUC_JP"),
        ("CharsetRecog_euc_kr", "COMMON_EUC_KR"),
        ("CharsetRecog_gb_18030", "COMMON_GB18030"),
    ]:
        arr = find_array(mchunks[cls], r"commonChars")
        assert arr, cls
        emit_ints(rust, arr, expect64=False)

    # 末尾只留一个换行:最后一块本身带 "\n",再拼一个就多出一行空白 ——
    # 于是「跑一遍生成器」会让入库的产品源码脏掉一行,而那正是 CI 的判据
    # (跑完全部生成器 git diff 必须为空)要抓的东西。
    OUT.write_text("\n".join(out).rstrip("\n") + "\n", encoding="utf-8")
    print(f"-> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
