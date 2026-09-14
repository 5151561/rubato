//! icu4j `CharsetDetector` 的移植(judge/engine 的 lib/icu4j,ICU 3.4 血统)。
//! 只移植 `EncodingDetect.getEncode` 用到的面:`setText(byte[]).detect()?.name`。
//!
//! 逐行对照移植;数据表由 tools/gen_icu4j_tables.py 自动抽取(icu4j_data.rs)。
//! 已知刻意保留的上游怪癖:
//! - gb18030 双字节第二位的 `secondByte >= 80`(**十进制** 80 = 0x50,上游笔误);
//! - IBM420/424 识别器默认关闭 → 不移植;
//! - 排序:Collections.sort(稳定,按 confidence 升序)后 reverse——
//!   同分时**注册序靠后的识别器优先**。

use crate::icu4j_data as d;

const K_BUF_SIZE: usize = 8000;

struct Det<'a> {
    raw: &'a [u8],
    /// fInputBytes/fInputLen(fStripTags=false:原样拷前 8000 字节)
    input: &'a [u8],
    c1_bytes: bool,
}

/// `CharsetDetector().setText(bytes).detect()` → 最佳匹配的 name
#[rustfmt::skip] // 手排的探测表:逐行对着 ICU CharsetDetector 的顺序
pub fn detect(bytes: &[u8]) -> Option<&'static str> {
    let input = &bytes[..bytes.len().min(K_BUF_SIZE)];
    let c1_bytes = input.iter().any(|&b| (0x80..=0x9f).contains(&b));
    let det = Det { raw: bytes, input, c1_bytes };

    // (confidence, name);顺序 = ALL_CS_RECOGNIZERS 注册序(默认启用的那些)
    let mut matches: Vec<(i32, &'static str)> = Vec::new();
    let mut push = |m: Option<(i32, &'static str)>| {
        if let Some(x) = m {
            matches.push(x);
        }
    };

    push(utf8(&det));
    push(utf16_be(&det));
    push(utf16_le(&det));
    push(utf32(&det, false));
    push(utf32(&det, true));
    push(mbcs(&det, next_char_sjis, Some(&d::COMMON_SJIS)).map(|c| (c, "Shift_JIS")));
    push(iso2022(&det, ESC_2022_JP).map(|c| (c, "ISO-2022-JP")));
    push(iso2022(&det, ESC_2022_CN).map(|c| (c, "ISO-2022-CN")));
    push(iso2022(&det, ESC_2022_KR).map(|c| (c, "ISO-2022-KR")));
    push(mbcs(&det, next_char_gb18030, Some(&d::COMMON_GB18030)).map(|c| (c, "GB18030")));
    push(mbcs(&det, next_char_euc, Some(&d::COMMON_EUC_JP)).map(|c| (c, "EUC-JP")));
    push(mbcs(&det, next_char_euc, Some(&d::COMMON_EUC_KR)).map(|c| (c, "EUC-KR")));
    push(mbcs(&det, next_char_big5, Some(&d::COMMON_BIG5)).map(|c| (c, "Big5")));
    push(sbcs_multi(
        &det,
        d::NGRAMS_8859_1,
        &d::BYTE_MAP_8859_1,
        if det.c1_bytes { "windows-1252" } else { "ISO-8859-1" },
    ));
    push(sbcs_multi(
        &det,
        d::NGRAMS_8859_2,
        &d::BYTE_MAP_8859_2,
        if det.c1_bytes { "windows-1250" } else { "ISO-8859-2" },
    ));
    push(sbcs_single(&det, &d::NGRAMS_8859_5_RU, &d::BYTE_MAP_8859_5, "ISO-8859-5"));
    push(sbcs_single(&det, &d::NGRAMS_8859_6_AR, &d::BYTE_MAP_8859_6, "ISO-8859-6"));
    push(sbcs_single(
        &det,
        &d::NGRAMS_8859_7_EL,
        &d::BYTE_MAP_8859_7,
        if det.c1_bytes { "windows-1253" } else { "ISO-8859-7" },
    ));
    push(sbcs_single(
        &det,
        &d::NGRAMS_8859_8_I_HE,
        &d::BYTE_MAP_8859_8,
        if det.c1_bytes { "windows-1255" } else { "ISO-8859-8-I" },
    ));
    push(sbcs_single(
        &det,
        &d::NGRAMS_8859_8_HE,
        &d::BYTE_MAP_8859_8,
        if det.c1_bytes { "windows-1255" } else { "ISO-8859-8" },
    ));
    push(sbcs_single(&det, &d::NGRAMS_WINDOWS_1251, &d::BYTE_MAP_WINDOWS_1251, "windows-1251"));
    push(sbcs_single(&det, &d::NGRAMS_WINDOWS_1256, &d::BYTE_MAP_WINDOWS_1256, "windows-1256"));
    push(sbcs_single(&det, &d::NGRAMS_KOI8_R, &d::BYTE_MAP_KOI8_R, "KOI8-R"));
    push(sbcs_single(
        &det,
        &d::NGRAMS_8859_9_TR,
        &d::BYTE_MAP_8859_9,
        if det.c1_bytes { "windows-1254" } else { "ISO-8859-9" },
    ));

    // Collections.sort(稳定升序)+ reverse
    matches.sort_by_key(|&(c, _)| c);
    matches.reverse();
    matches.first().map(|&(_, n)| n)
}

// ---- UTF-8 ----

fn utf8(det: &Det) -> Option<(i32, &'static str)> {
    let input = det.raw;
    let len = input.len();
    let has_bom = len >= 3 && input[0] == 0xef && input[1] == 0xbb && input[2] == 0xbf;
    let mut num_valid: i32 = 0;
    let mut num_invalid: i32 = 0;
    let mut i = 0usize;
    while i < len {
        let b = input[i];
        if b & 0x80 == 0 {
            i += 1;
            continue;
        }
        let mut trail_bytes;
        if b & 0xe0 == 0xc0 {
            trail_bytes = 1;
        } else if b & 0xf0 == 0xe0 {
            trail_bytes = 2;
        } else if b & 0xf8 == 0xf0 {
            trail_bytes = 3;
        } else {
            num_invalid += 1;
            i += 1;
            continue;
        }
        loop {
            i += 1;
            if i >= len {
                break;
            }
            let b = input[i];
            if b & 0xc0 != 0x80 {
                num_invalid += 1;
                break;
            }
            trail_bytes -= 1;
            if trail_bytes == 0 {
                num_valid += 1;
                break;
            }
        }
        i += 1;
    }
    let confidence = if has_bom && num_invalid == 0 {
        100
    } else if has_bom && num_valid > num_invalid * 10 {
        80
    } else if num_valid > 3 && num_invalid == 0 {
        100
    } else if num_valid > 0 && num_invalid == 0 {
        80
    } else if num_valid == 0 && num_invalid == 0 {
        15
    } else if num_valid > num_invalid * 10 {
        25
    } else {
        0
    };
    (confidence != 0).then_some((confidence, "UTF-8"))
}

// ---- UTF-16 / UTF-32 ----

fn adjust_confidence(code_unit: u32, mut confidence: i32) -> i32 {
    if code_unit == 0 {
        confidence -= 10;
    } else if (0x20..=0xff).contains(&code_unit) || code_unit == 0x0a {
        confidence += 10;
    }
    confidence.clamp(0, 100)
}

fn utf16(det: &Det, le: bool) -> Option<i32> {
    let input = det.raw;
    let mut confidence: i32 = 10;
    let bytes_to_check = input.len().min(30);
    let mut char_index = 0usize;
    while char_index + 1 < bytes_to_check {
        let (hi, lo) = if le {
            (input[char_index + 1], input[char_index])
        } else {
            (input[char_index], input[char_index + 1])
        };
        let code_unit = ((hi as u32) << 8) | lo as u32;
        if char_index == 0 && code_unit == 0xfeff {
            confidence = 100;
            break;
        }
        confidence = adjust_confidence(code_unit, confidence);
        if confidence == 0 || confidence == 100 {
            break;
        }
        char_index += 2;
    }
    if bytes_to_check < 4 && confidence < 100 {
        confidence = 0;
    }
    (confidence > 0).then_some(confidence)
}

fn utf16_be(det: &Det) -> Option<(i32, &'static str)> {
    utf16(det, false).map(|c| (c, "UTF-16BE"))
}

fn utf16_le(det: &Det) -> Option<(i32, &'static str)> {
    utf16(det, true).map(|c| (c, "UTF-16LE"))
}

fn utf32(det: &Det, le: bool) -> Option<(i32, &'static str)> {
    let input = det.raw;
    let limit = (input.len() / 4) * 4;
    if limit == 0 {
        return None;
    }
    let get_char = |i: usize| -> i64 {
        let b = |k: usize| input[i + k] as i64;
        // Java int 溢出语义:按 32 位有符号回绕
        let v = if le {
            (b(3) << 24) | (b(2) << 16) | (b(1) << 8) | b(0)
        } else {
            (b(0) << 24) | (b(1) << 16) | (b(2) << 8) | b(3)
        };
        v as i32 as i64
    };
    let has_bom = get_char(0) == 0x0000feff;
    let mut num_valid = 0i32;
    let mut num_invalid = 0i32;
    let mut i = 0;
    while i < limit {
        let ch = get_char(i);
        if !(0..0x10ffff).contains(&ch) || (0xd800..=0xdfff).contains(&ch) {
            num_invalid += 1;
        } else {
            num_valid += 1;
        }
        i += 4;
    }
    let confidence = if has_bom && num_invalid == 0 {
        100
    } else if has_bom && num_valid > num_invalid * 10 {
        80
    } else if num_valid > 3 && num_invalid == 0 {
        100
    } else if num_valid > 0 && num_invalid == 0 {
        80
    } else if num_valid > num_invalid * 10 {
        25
    } else {
        0
    };
    (confidence != 0).then_some((confidence, if le { "UTF-32LE" } else { "UTF-32BE" }))
}

// ---- ISO-2022 家族 ----

const ESC_2022_JP: &[&[u8]] = &[
    &[0x1b, 0x24, 0x28, 0x43],
    &[0x1b, 0x24, 0x28, 0x44],
    &[0x1b, 0x24, 0x40],
    &[0x1b, 0x24, 0x41],
    &[0x1b, 0x24, 0x42],
    &[0x1b, 0x26, 0x40],
    &[0x1b, 0x28, 0x42],
    &[0x1b, 0x28, 0x48],
    &[0x1b, 0x28, 0x49],
    &[0x1b, 0x28, 0x4a],
    &[0x1b, 0x2e, 0x41],
    &[0x1b, 0x2e, 0x46],
];
const ESC_2022_KR: &[&[u8]] = &[&[0x1b, 0x24, 0x29, 0x43]];
const ESC_2022_CN: &[&[u8]] = &[
    &[0x1b, 0x24, 0x29, 0x41],
    &[0x1b, 0x24, 0x29, 0x47],
    &[0x1b, 0x24, 0x2a, 0x48],
    &[0x1b, 0x24, 0x29, 0x45],
    &[0x1b, 0x24, 0x2b, 0x49],
    &[0x1b, 0x24, 0x2b, 0x4a],
    &[0x1b, 0x24, 0x2b, 0x4b],
    &[0x1b, 0x24, 0x2b, 0x4c],
    &[0x1b, 0x24, 0x2b, 0x4d],
    &[0x1b, 0x4e],
    &[0x1b, 0x4f],
];

fn iso2022(det: &Det, escape_sequences: &[&[u8]]) -> Option<i32> {
    let text = det.input;
    let text_len = text.len();
    let mut hits = 0i32;
    let mut misses = 0i32;
    let mut shifts = 0i32;
    let mut i = 0usize;
    'scan: while i < text_len {
        if text[i] == 0x1b {
            'check: for seq in escape_sequences {
                if text_len - i < seq.len() {
                    continue;
                }
                for j in 1..seq.len() {
                    if seq[j] != text[i + j] {
                        continue 'check;
                    }
                }
                hits += 1;
                i += seq.len() - 1;
                i += 1;
                continue 'scan;
            }
            misses += 1;
        }
        if text[i] == 0x0e || text[i] == 0x0f {
            shifts += 1;
        }
        i += 1;
    }
    if hits == 0 {
        return None;
    }
    let mut quality = (100 * hits - 100 * misses) / (hits + misses);
    if hits + shifts < 5 {
        quality -= (5 - (hits + shifts)) * 10;
    }
    if quality < 0 {
        quality = 0;
    }
    (quality != 0).then_some(quality)
}

// ---- mbcs 家族 ----

struct IterChar {
    char_value: i64,
    next_index: usize,
    error: bool,
    done: bool,
}

impl IterChar {
    fn next_byte(&mut self, det: &Det) -> i32 {
        if self.next_index >= det.raw.len() {
            self.done = true;
            return -1;
        }
        let b = det.raw[self.next_index] as i32;
        self.next_index += 1;
        b
    }
}

type NextChar = fn(&mut IterChar, &Det) -> bool;

fn mbcs(det: &Det, next_char: NextChar, common_chars: Option<&[i32]>) -> Option<i32> {
    let mut double_byte_char_count = 0i32;
    let mut common_char_count = 0i32;
    let mut bad_char_count = 0i32;
    let mut total_char_count = 0i32;
    let mut confidence: i32;

    let mut iter = IterChar { char_value: 0, next_index: 0, error: false, done: false };
    'detect: {
        while next_char(&mut iter, det) {
            total_char_count += 1;
            if iter.error {
                bad_char_count += 1;
            } else {
                let cv = iter.char_value & 0xffff_ffff;
                if cv > 0xff {
                    double_byte_char_count += 1;
                    if let Some(cc) = common_chars {
                        if cc.binary_search(&(cv as i32)).is_ok() {
                            common_char_count += 1;
                        }
                    }
                }
            }
            if bad_char_count >= 2 && bad_char_count * 5 >= double_byte_char_count {
                confidence = 0;
                break 'detect;
            }
        }

        if double_byte_char_count <= 10 && bad_char_count == 0 {
            confidence = if double_byte_char_count == 0 && total_char_count < 10 { 0 } else { 10 };
            break 'detect;
        }

        if double_byte_char_count < 20 * bad_char_count {
            confidence = 0;
            break 'detect;
        }

        match common_chars {
            None => {
                confidence = (30 + double_byte_char_count - 20 * bad_char_count).min(100);
            }
            Some(_) => {
                // Java:`Math.log((float) doubleByteCharCount / 4)`——float 除法后取对数
                let max_val = ((double_byte_char_count as f32 / 4.0f32) as f64).ln();
                let scale_factor = 90.0 / max_val;
                confidence = (((common_char_count as f64) + 1.0).ln() * scale_factor + 10.0) as i32;
                confidence = confidence.min(100);
            }
        }
    }
    (confidence != 0).then_some(confidence)
}

fn next_char_sjis(it: &mut IterChar, det: &Det) -> bool {
    it.error = false;
    let first_byte = it.next_byte(det);
    it.char_value = first_byte as i64;
    if first_byte < 0 {
        return false;
    }
    if first_byte <= 0x7f || (first_byte > 0xa0 && first_byte <= 0xdf) {
        return true;
    }
    let second_byte = it.next_byte(det);
    if second_byte < 0 {
        return false;
    }
    it.char_value = ((first_byte as i64) << 8) | second_byte as i64;
    if !((0x40..=0x7f).contains(&second_byte) || (0x80..=0xff).contains(&second_byte)) {
        it.error = true;
    }
    true
}

fn next_char_big5(it: &mut IterChar, det: &Det) -> bool {
    it.error = false;
    let first_byte = it.next_byte(det);
    it.char_value = first_byte as i64;
    if first_byte < 0 {
        return false;
    }
    if first_byte <= 0x7f || first_byte == 0xff {
        return true;
    }
    let second_byte = it.next_byte(det);
    if second_byte < 0 {
        return false;
    }
    it.char_value = (it.char_value << 8) | second_byte as i64;
    if second_byte < 0x40 || second_byte == 0x7f || second_byte == 0xff {
        it.error = true;
    }
    true
}

fn next_char_euc(it: &mut IterChar, det: &Det) -> bool {
    it.error = false;
    'build: {
        let first_byte = it.next_byte(det);
        it.char_value = first_byte as i64;
        if first_byte < 0 {
            it.done = true;
            break 'build;
        }
        if first_byte <= 0x8d {
            break 'build;
        }
        let second_byte = it.next_byte(det);
        it.char_value = (it.char_value << 8) | second_byte as i64;
        if (0xa1..=0xfe).contains(&first_byte) {
            if second_byte < 0xa1 {
                it.error = true;
            }
            break 'build;
        }
        if first_byte == 0x8e {
            if second_byte < 0xa1 {
                it.error = true;
            }
            break 'build;
        }
        if first_byte == 0x8f {
            let third_byte = it.next_byte(det);
            it.char_value = (it.char_value << 8) | third_byte as i64;
            if third_byte < 0xa1 {
                it.error = true;
            }
        }
    }
    !it.done
}

fn next_char_gb18030(it: &mut IterChar, det: &Det) -> bool {
    it.error = false;
    'build: {
        let first_byte = it.next_byte(det);
        it.char_value = first_byte as i64;
        if first_byte < 0 {
            it.done = true;
            break 'build;
        }
        if first_byte <= 0x80 {
            break 'build;
        }
        let second_byte = it.next_byte(det);
        it.char_value = (it.char_value << 8) | second_byte as i64;
        if (0x81..=0xfe).contains(&first_byte) {
            // 上游原文:`secondByte >= 80`(十进制!)——保留笔误
            if (0x40..=0x7e).contains(&second_byte) || (80..=0xfe).contains(&second_byte) {
                break 'build;
            }
            if (0x30..=0x39).contains(&second_byte) {
                let third_byte = it.next_byte(det);
                if (0x81..=0xfe).contains(&third_byte) {
                    let fourth_byte = it.next_byte(det);
                    if (0x30..=0x39).contains(&fourth_byte) {
                        it.char_value =
                            (it.char_value << 16) | ((third_byte as i64) << 8) | fourth_byte as i64;
                        break 'build;
                    }
                }
            }
            it.error = true;
        }
    }
    !it.done
}

// ---- sbcs(ngram)家族 ----

fn ngram_parse(det: &Det, ngram_list: &[i32; 64], byte_map: &[u8; 256]) -> i32 {
    let space_char: u8 = 0x20;
    let mut ngram: i32 = 0;
    let mut ngram_count: i64 = 0;
    let mut hit_count: i64 = 0;
    let mut add_byte = |b: u8| {
        ngram = ((ngram << 8) + (b as i32)) & 0x00ff_ffff;
        ngram_count += 1;
        // `NGramParser.search`:真身是 64 项表的手写二分(index 可到 -1),
        // 表严格升序(tests::ngram_tables_strictly_ascending 断言)时
        // 等价于有序表 membership
        if ngram_list.binary_search(&ngram).is_ok() {
            hit_count += 1;
        }
    };

    let mut ignore_space = false;
    for &raw in det.input {
        let mb = byte_map[raw as usize];
        if mb != 0 {
            if !(mb == space_char && ignore_space) {
                add_byte(mb);
            }
            ignore_space = mb == space_char;
        }
    }
    add_byte(space_char);

    let raw_percent = hit_count as f64 / ngram_count as f64;
    if raw_percent > 0.33 {
        return 98;
    }
    (raw_percent * 300.0) as i32
}

fn sbcs_single(
    det: &Det,
    ngrams: &[i32; 64],
    byte_map: &[u8; 256],
    name: &'static str,
) -> Option<(i32, &'static str)> {
    let confidence = ngram_parse(det, ngrams, byte_map);
    (confidence != 0).then_some((confidence, name))
}

/// 8859-1/8859-2:多语言 ngram 取最高分;`<= 0` 才算无匹配(与单语言的 `== 0` 不同)
fn sbcs_multi(
    det: &Det,
    groups: &[(&str, &[i32; 64])],
    byte_map: &[u8; 256],
    name: &'static str,
) -> Option<(i32, &'static str)> {
    let mut best: i32 = -1;
    for (_lang, ngrams) in groups {
        let confidence = ngram_parse(det, ngrams, byte_map);
        if confidence > best {
            best = confidence;
        }
    }
    (best > 0).then_some((best, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_utf8() {
        assert_eq!(detect("第一章 这是一段没有标记的中文正文。".as_bytes()), Some("UTF-8"));
    }

    #[test]
    fn detects_gb18030() {
        let (bytes, _, _) = encoding_rs::GBK
            .encode("第一章 风雪山神庙 林冲在山神庙里避雪,忽然听到外面有人说话,便侧耳倾听起来。");
        assert_eq!(detect(&bytes), Some("GB18030"));
    }

    #[test]
    fn detects_big5() {
        let (bytes, _, _) = encoding_rs::BIG5
            .encode("第一章 這是一段繁體中文的內容,用來測試字符集偵測的行為是否一致。");
        assert_eq!(detect(&bytes), Some("Big5"));
    }

    /// ngram 命中从手写二分换成 `binary_search` 的前提:所有表严格升序无重复
    #[test]
    fn ngram_tables_strictly_ascending() {
        let singles: &[&[i32; 64]] = &[
            &d::NGRAMS_8859_5_RU,
            &d::NGRAMS_8859_6_AR,
            &d::NGRAMS_8859_7_EL,
            &d::NGRAMS_8859_8_I_HE,
            &d::NGRAMS_8859_8_HE,
            &d::NGRAMS_8859_9_TR,
            &d::NGRAMS_WINDOWS_1251,
            &d::NGRAMS_WINDOWS_1256,
            &d::NGRAMS_KOI8_R,
        ];
        let all = singles
            .iter()
            .copied()
            .chain(d::NGRAMS_8859_1.iter().map(|(_, t)| *t))
            .chain(d::NGRAMS_8859_2.iter().map(|(_, t)| *t));
        for table in all {
            assert!(table.windows(2).all(|w| w[0] < w[1]), "表须严格升序:{table:?}");
        }
    }

    #[test]
    fn ascii_is_latin1_or_utf8() {
        // 纯 ASCII:UTF-8 给 15,8859-1(en)按 ngram 命中率可能更高——
        // 具体赢家以裁判差分为准,这里只保证不崩且有结果
        assert!(detect(b"the quick brown fox jumps over the lazy dog and runs away").is_some());
    }
}
