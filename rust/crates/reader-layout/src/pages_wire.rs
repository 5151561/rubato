//! 页表的**跨 FFI 线格式**(Rust → Dart)。
//!
//! **格式(小端)**
//!
//! ```text
//! 头     : u16 schema_version | u16 flags(bit0 = complete)
//!          u32 page_count | u32 measured_blocks
//!          u32 diag_oversized_lines | u32 diag_merged_pages
//! 每页   : u32 source_start | u32 source_end | u32 placement_count | u32 reserved
//! 每摆放 : u32 kind | f32 line_gap | u64 block_id
//!          u32 line_start | u32 line_end | f32 origin_x | f32 origin_y
//! ```
//!
//! `page_index` 是页的下标,不占位。**`kind` 位现在恒为 0(行段)**——
//! 它是 M2 的图片、M4 的高亮进来时的接口:那时是加一个 kind 值,
//! 不是改协议(计划书 §5.4)。Dart 读到不认识的 kind 必须整份拒绝。

use crate::compose::{ComposeResult, Placement};

/// 行段。M2/M4 加变体时在这里续号,旧值不动
pub const KIND_LINES: u32 = 0;

pub const PAGES_WIRE_HEADER_BYTES: usize = 20;

pub fn encode_pages(r: &ComposeResult) -> Vec<u8> {
    let mut out = Vec::with_capacity(PAGES_WIRE_HEADER_BYTES + r.pages.len() * 48);
    out.extend_from_slice(&crate::MEASURE_SCHEMA_VERSION.to_le_bytes());
    out.extend_from_slice(&(u16::from(r.complete)).to_le_bytes());
    out.extend_from_slice(&(r.pages.len() as u32).to_le_bytes());
    out.extend_from_slice(&r.measured_blocks.to_le_bytes());
    out.extend_from_slice(&r.diagnostics.oversized_lines.to_le_bytes());
    out.extend_from_slice(&r.diagnostics.merged_pages.to_le_bytes());
    for p in &r.pages {
        out.extend_from_slice(&p.source_start.to_le_bytes());
        out.extend_from_slice(&p.source_end.to_le_bytes());
        out.extend_from_slice(&(p.placements.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for pl in &p.placements {
            match pl {
                Placement::Lines {
                    block_id,
                    line_start,
                    line_end,
                    origin_x,
                    origin_y,
                    line_gap,
                } => {
                    out.extend_from_slice(&KIND_LINES.to_le_bytes());
                    // v1 那个 reserved 位换成了 `line_gap`(M3d 的页底对齐)——
                    // 位没变、语义变了,所以升版本
                    out.extend_from_slice(&line_gap.to_bits().to_le_bytes());
                    out.extend_from_slice(&block_id.to_le_bytes());
                    out.extend_from_slice(&line_start.to_le_bytes());
                    out.extend_from_slice(&line_end.to_le_bytes());
                    out.extend_from_slice(&origin_x.to_bits().to_le_bytes());
                    out.extend_from_slice(&origin_y.to_bits().to_le_bytes());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::{ComposeDiagnostics, PagePlan};

    #[test]
    fn 头位与长度自洽() {
        let r = ComposeResult {
            pages: vec![PagePlan {
                page_index: 0,
                source_start: 0,
                source_end: 12,
                placements: vec![Placement::Lines {
                    block_id: 0,
                    line_start: 0,
                    line_end: 2,
                    origin_x: 0.0,
                    origin_y: -3.5,
                    line_gap: 1.25,
                }],
            }],
            measured_blocks: 1,
            complete: true,
            diagnostics: ComposeDiagnostics::default(),
        };
        let buf = encode_pages(&r);
        assert_eq!(buf.len(), PAGES_WIRE_HEADER_BYTES + 16 + 32);
        assert_eq!(u16::from_le_bytes(buf[2..4].try_into().expect("2")), 1, "complete 位");
        assert_eq!(u32::from_le_bytes(buf[4..8].try_into().expect("4")), 1);
        let at = PAGES_WIRE_HEADER_BYTES + 16;
        assert_eq!(u32::from_le_bytes(buf[at..at + 4].try_into().expect("4")), KIND_LINES);
        let gap =
            f32::from_bits(u32::from_le_bytes(buf[at + 4..at + 8].try_into().expect("4")));
        assert_eq!(gap, 1.25, "line_gap 占的是 v1 那个 reserved 位");
        let oy = f32::from_bits(u32::from_le_bytes(
            buf[at + 28..at + 32].try_into().expect("4"),
        ));
        assert_eq!(oy, -3.5);
    }
}
