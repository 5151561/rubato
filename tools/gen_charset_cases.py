#!/usr/bin/env python3
"""charset 差分(EncodingDetect.getHtmlEncode = meta 判定 + icu4j 检测)的用例生成。

输入是 base64 字节,输出是两侧的 charset 名字符串。覆盖:
- 各语言样本 × 各编码的裸字节(icu4j 检测路径);
- HTML 包装(有/无/坏 meta,大小写 head,>8000 字节窗口);
- 截断(多字节序列截半)、BOM、纯 ASCII、空、随机字节。
"""
import base64
import json
import random
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "fixtures" / "cases" / "charset"

TEXTS = {
    "zh_hans": "第一章 风雪山神庙 林冲在山神庙里避雪,忽然听到外面有人低声说话,便侧耳倾听。"
               "那雪下得正紧,北风卷着碎琼乱玉扑进破庙。他将花枪倚在供桌旁,取下毡笠抖落积雪,"
               "又摸出怀里的冷酒喝了两口,只觉浑身暖了几分。庙外脚步声渐近,竟是三个人踏雪而来。",
    "zh_hant": "第一章 這是一段繁體中文的內容,用來測試字符集偵測的行為是否一致。"
               "夜色漸深,城外的燈火一盞一盞熄滅,只剩下更夫的梆子聲在長街上迴盪。"
               "他推開木門,案上的書卷還攤著,墨跡未乾,窗外忽然傳來一聲極輕的瓦響。",
    "ja": "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ。"
          "何でも薄暗いじめじめした所でニャーニャー泣いていた事だけは記憶している。"
          "吾輩はここで始めて人間というものを見た。しかもあとで聞くとそれは書生という、"
          "人間中で一番獰悪な種族であったそうだ。",
    "ko": "모든 국민은 인간으로서의 존엄과 가치를 가지며, 행복을 추구할 권리를 가진다. "
          "국가는 개인이 가지는 불가침의 기본적 인권을 확인하고 이를 보장할 의무를 진다. "
          "누구든지 법률에 의하지 아니하고는 체포 또는 구속을 당하지 아니한다.",
    "ru": "В начале июля, в чрезвычайно жаркое время, под вечер, один молодой человек "
          "вышел из своей каморки, которую нанимал от жильцов, на улицу и медленно, "
          "как бы в нерешимости, отправился к мосту. Он благополучно избегнул встречи "
          "с своею хозяйкой на лестнице.",
    "el": "Η ελληνική γλώσσα είναι μία από τις αρχαιότερες γλώσσες του κόσμου και "
          "ομιλείται συνεχώς εδώ και τουλάχιστον τρεις χιλιάδες χρόνια. Η ιστορία της "
          "χωρίζεται σε περιόδους και κάθε περίοδος έχει τα δικά της χαρακτηριστικά.",
    "tr": "Bütün insanlar hür, haysiyet ve haklar bakımından eşit doğarlar. Akıl ve "
          "vicdana sahiptirler ve birbirlerine karşı kardeşlik zihniyeti ile hareket "
          "etmelidirler. Herkes ırk, renk, cinsiyet, dil, din ayrımı gözetilmeksizin "
          "bu bildiride yazılı bütün haklardan yararlanabilir.",
    "ar": "يولد جميع الناس أحرارا متساوين في الكرامة والحقوق. وقد وهبوا عقلا وضميرا "
          "وعليهم أن يعامل بعضهم بعضا بروح الإخاء. لكل إنسان حق التمتع بكافة الحقوق "
          "والحريات الواردة في هذا الإعلان دون أي تمييز.",
    "he": "כל בני האדם נולדו בני חורין ושווים בערכם ובזכויותיהם. כולם חוננו בתבונה "
          "ובמצפון, לפיכך חובה עליהם לנהוג איש ברעהו ברוח של אחווה. כל אדם זכאי "
          "לזכויות ולחירויות שנקבעו בהכרזה זו.",
    "fr": "Tous les êtres humains naissent libres et égaux en dignité et en droits. "
          "Ils sont doués de raison et de conscience et doivent agir les uns envers "
          "les autres dans un esprit de fraternité. Chacun peut se prévaloir de tous "
          "les droits et de toutes les libertés proclamés dans la présente déclaration.",
    "de": "Alle Menschen sind frei und gleich an Würde und Rechten geboren. Sie sind "
          "mit Vernunft und Gewissen begabt und sollen einander im Geist der "
          "Brüderlichkeit begegnen. Jeder hat Anspruch auf die in dieser Erklärung "
          "verkündeten Rechte und Freiheiten ohne irgendeinen Unterschied.",
    "cs": "Všichni lidé rodí se svobodní a sobě rovní co do důstojnosti a práv. Jsou "
          "nadáni rozumem a svědomím a mají spolu jednat v duchu bratrství. Každý má "
          "všechna práva a všechny svobody, stanovené touto deklarací.",
}

# (文本键, python 编码名) —— 每对生成裸字节 + 变体
PAIRS = [
    ("zh_hans", "gb18030"), ("zh_hans", "gbk"), ("zh_hans", "utf-8"),
    ("zh_hant", "big5"), ("zh_hant", "utf-8"),
    ("ja", "shift_jis"), ("ja", "euc_jp"), ("ja", "iso2022_jp"), ("ja", "utf-8"),
    ("ko", "euc_kr"), ("ko", "utf-8"),
    ("ru", "koi8_r"), ("ru", "cp1251"), ("ru", "iso8859_5"), ("ru", "utf-8"),
    ("el", "iso8859_7"), ("el", "cp1253"),
    ("tr", "iso8859_9"), ("tr", "cp1254"),
    ("ar", "iso8859_6"), ("ar", "cp1256"),
    ("he", "iso8859_8"), ("he", "cp1255"),
    ("fr", "iso8859_1"), ("fr", "cp1252"), ("fr", "utf-8"),
    ("de", "iso8859_1"), ("de", "utf-8"),
    ("cs", "iso8859_2"), ("cs", "cp1250"),
    ("zh_hans", "utf-16le"), ("zh_hans", "utf-16be"),
    ("fr", "utf-16le"), ("fr", "utf-16be"),
    ("zh_hans", "utf-32le"), ("zh_hans", "utf-32be"),
]

cases = []


def case(bytes_, tag):
    cases.append({
        "id": f"cd{len(cases):05d}",
        "op": "detectCharset",
        "tag": tag,  # 只为可读性,不进比较(两侧都原样忽略未知字段)
        "bytesBase64": base64.b64encode(bytes_).decode(),
    })


def main():
    for key, enc in PAIRS:
        text = TEXTS[key]
        b = text.encode(enc, errors="replace")
        case(b, f"{key}/{enc}/raw")
        # HTML 包装,无 meta:走检测
        html = f"<html><head><title>t</title></head><body><p>{text}</p></body></html>"
        case(html.encode(enc, errors="replace"), f"{key}/{enc}/html")
        # 截断(可能切在多字节中间)
        case(b[: len(b) * 2 // 3 + 1], f"{key}/{enc}/trunc")
        # 短样本(置信度低区)
        case(b[:24], f"{key}/{enc}/short")

    # meta 路径(值原样返回,包括坏值);大小写 head;head 超窗口位置无关
    zh = TEXTS["zh_hans"]
    for meta, tag in [
        ('<meta charset="gbk">', "meta/charset"),
        ('<meta http-equiv="content-type" content="text/html; charset=big5">', "meta/http-equiv"),
        ('<meta charset="NoSuchCharset">', "meta/bad-name"),
        ('<meta charset="">', "meta/empty→detect"),
        ('<meta http-equiv="Content-Type" content="text/html">', "meta/no-charset→detect"),
        ('<meta http-equiv="refresh" content="1;url=x">', "meta/other-equiv→detect"),
    ]:
        html = f"<html><head>{meta}</head><body>{zh}</body></html>"
        case(html.encode("gbk"), f"{tag}/gbk-bytes")
    case(f"<HTML><HEAD><META CHARSET=gbk></HEAD><body>{zh}</body></HTML>".encode("gbk"),
         "meta/upper-head")
    case(f"<head><meta charset='utf-8'></head>{zh}".encode("utf-8"), "meta/single-quote")
    # 只有 <head> 无闭合 → 正则也不中 → 检测
    case(f"<html><head><meta charset=gbk><body>{zh}".encode("gbk"), "meta/unclosed-head")

    # 特殊字节
    case(b"", "empty")
    case(b"a", "single-ascii")
    case(b"\x00", "single-nul")
    case("中".encode("utf-8")[:2], "utf8-partial")
    case(b"\xef\xbb\xbf", "bare-utf8-bom")
    case(b"\xef\xbb\xbfhello world this is ascii", "utf8-bom-ascii")
    case("﻿你好世界".encode("utf-16le"), "utf16le-bom")
    case("﻿你好世界".encode("utf-16be"), "utf16be-bom")
    case("﻿你好".encode("utf-32le"), "utf32le-bom")
    case("﻿你好".encode("utf-32be"), "utf32be-bom")
    case(b"the quick brown fox jumps over the lazy dog. " * 8, "pure-ascii-long")
    case(b"<a><b><c><d><e><f>" * 40, "markup-only")

    # 窗口边界:前 8000 字节干净 GBK,其后接乱字节(sbcs/2022 只看前 8000,
    # utf8/mbcs 看全量——行为差异要两侧一致)
    clean = (zh * 40).encode("gbk")[:8000]
    rng = random.Random(20260829)
    junk = bytes(rng.randrange(256) for _ in range(2000))
    case(clean + junk, "window/clean8k-then-junk")
    case(clean, "window/clean8k")

    # 随机字节 fuzz(种子固定)
    for i in range(80):
        n = rng.choice([3, 7, 17, 64, 256, 1024, 5000])
        case(bytes(rng.randrange(256) for _ in range(n)), f"fuzz/{i}")
    # 高位随机(逼出 sbcs/mbcs 分支)
    for i in range(40):
        n = rng.choice([32, 200, 2000])
        case(bytes(rng.randrange(0x80, 0x100) if rng.random() < 0.7 else rng.randrange(0x20, 0x7f)
                   for _ in range(n)), f"fuzz-hi/{i}")

    # 真实页面字节(fixtures/pages)+ 它们的 GBK 转写
    for f in sorted((ROOT / "fixtures" / "pages").glob("*.html")):
        raw = f.read_bytes()
        case(raw[:60000], f"page/{f.name}")
        try:
            case(raw.decode("utf-8").encode("gbk", errors="replace")[:60000],
                 f"page/{f.name}/gbk")
        except Exception:
            pass

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "generated.json").write_text(
        json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(cases)} charset cases -> {OUT/'generated.json'}", file=sys.stderr)


if __name__ == "__main__":
    main()
