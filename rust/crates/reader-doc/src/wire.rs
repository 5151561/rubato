//! 文档模型的**跨 FFI 线格式**:一段自描述字节缓冲。
//!
//! 与 `reader_layout::measure` 同一套理由(计划书 §5.3):一章两三千个块,
//! 不为每个建跨 FFI 小对象。Dart 侧用 `ByteData` 顺序读。
//!
//! **格式(小端)**
//!
//! ```text
//! 头   : u16 schema_version | u16 flags(0) | u32 block_count | u32 source_utf16_len | u32 reserved
//! 每块 : u32 kind | u32 source_start | u32 text_end | u32 source_end | u32 indent_len
//!        u32 src_start | u32 src_end
//! ```
//!
//! `kind` 的取值见 [`crate::BlockKind::wire`]。Dart 那一半读到**不认识的种类
//! 必须整份拒绝** —— 拿段落的样式去量一个图片块,量出来的是一段能画、
//! 但完全不对的东西,比不画更难查。
//!
//! `src_start` / `src_end` 只对图片块有意义(其余块两端相等):Dart 按它切出
//! 图片 URL,不自己再写一遍标签解析(见 [`crate::Block::src_start`])。
//!
//! 块的下标即 `block_id`([`crate::Block::id`] 就是块序号),所以不占位。

use crate::{Block, ChapterDocument};

pub const BLOCK_WIRE_HEADER_BYTES: usize = 16;
pub const BLOCK_WIRE_RECORD_BYTES: usize = 28;

pub fn encode_blocks(doc: &ChapterDocument) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(BLOCK_WIRE_HEADER_BYTES + doc.blocks.len() * BLOCK_WIRE_RECORD_BYTES);
    out.extend_from_slice(&doc.schema_version.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(doc.blocks.len() as u32).to_le_bytes());
    out.extend_from_slice(&doc.source_utf16_len.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for b in &doc.blocks {
        push_block(&mut out, b);
    }
    out
}

fn push_block(out: &mut Vec<u8>, b: &Block) {
    out.extend_from_slice(&b.kind.wire().to_le_bytes());
    out.extend_from_slice(&b.source_start.get().to_le_bytes());
    out.extend_from_slice(&b.text_end.get().to_le_bytes());
    out.extend_from_slice(&b.source_end.get().to_le_bytes());
    out.extend_from_slice(&b.indent_len.to_le_bytes());
    out.extend_from_slice(&b.src_start.get().to_le_bytes());
    out.extend_from_slice(&b.src_end.get().to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 长度自洽() {
        let doc = ChapterDocument::build("t", "甲\n乙\n丙").expect("doc");
        let buf = encode_blocks(&doc);
        assert_eq!(buf.len(), BLOCK_WIRE_HEADER_BYTES + 3 * BLOCK_WIRE_RECORD_BYTES);
        assert_eq!(u32::from_le_bytes(buf[4..8].try_into().expect("4")), 3);
        assert_eq!(
            u32::from_le_bytes(buf[8..12].try_into().expect("4")),
            doc.source_utf16_len
        );
        // 第二块的 kind 与 source_start
        let at = BLOCK_WIRE_HEADER_BYTES + BLOCK_WIRE_RECORD_BYTES;
        assert_eq!(u32::from_le_bytes(buf[at..at + 4].try_into().expect("4")), 0);
        assert_eq!(
            u32::from_le_bytes(buf[at + 4..at + 8].try_into().expect("4")),
            doc.blocks[1].source_start.get()
        );
    }

    /// 图片块的种类位与 src 区间都进了线格式
    #[test]
    fn 图片块的种类位与_src_区间都写进了线格式() {
        let doc = ChapterDocument::build("t", "甲\n　　<img src=\"http://a/1.jpg\">").expect("doc");
        let buf = encode_blocks(&doc);
        let at = BLOCK_WIRE_HEADER_BYTES + BLOCK_WIRE_RECORD_BYTES;
        let u32_at = |o: usize| u32::from_le_bytes(buf[at + o..at + o + 4].try_into().expect("4"));
        assert_eq!(u32_at(0), 2, "第二块是图片");
        assert_eq!(u32_at(4), u32_at(8), "render 文本为空:text_end == source_start");
        let (s, e) = (u32_at(20) as usize, u32_at(24) as usize);
        let text: Vec<u16> = "甲\n　　<img src=\"http://a/1.jpg\">".encode_utf16().collect();
        assert_eq!(String::from_utf16(&text[s..e]).expect("utf16"), "http://a/1.jpg");
    }

    #[test]
    fn 标题块的种类位写进了线格式() {
        let doc = ChapterDocument::build_with_title("t", "第一章", "甲\n乙").expect("doc");
        let buf = encode_blocks(&doc);
        assert_eq!(u32::from_le_bytes(buf[16..20].try_into().expect("4")), 1, "首块是标题");
        let at = BLOCK_WIRE_HEADER_BYTES + BLOCK_WIRE_RECORD_BYTES;
        assert_eq!(u32::from_le_bytes(buf[at..at + 4].try_into().expect("4")), 0, "次块是段落");
    }
}
