//! Dart → Rust 的**行度量批次**:一段自描述的字节缓冲。
//!
//! 计划书 §5.3:一章几千到上万条行度量,不能为每条建一个跨 FFI 小对象。
//! 这里把它们打成一个 `Vec<u8>`(FRB 2.13 对 `Vec<u8>` 有 strict typed-list
//! 通道),Dart 侧用 `ByteData` 顺序写、这边顺序读。
//!
//! **格式(小端,顺序读)**
//!
//! ```text
//! 头   : u16 schema_version | u16 flags(0) | u32 block_count
//! 每块 : u64 block_id | u32 line_count | f32 space_before
//!        紧跟本块的 line_count 条行度量
//! 每行 : u32 render_start | u32 render_end | f32 top | f32 height
//! ```
//!
//! # 为什么段距是**每块一个数**,不是装页参数
//!
//! v1 的段距是 [`crate::ComposeParams`] 上的一个全局 `block_spacing`。M2a 的
//! 「标题/正文样式分离」一来它就不够用了:标题的上间距、下间距与段距是三个
//! 不同的数,而选哪个取决于**前一块和这一块各是什么种类** —— 那是样式问题,
//! 样式在 Dart。
//!
//! 所以这里收的是**已经算好的一个数**:Dart 按 (前一块种类, 本块种类) 定它,
//! 装页器只管加。装页器不认识「标题」,也就不会因为多一种块而改一次。
//! v1 那个位置是本块头里的 `reserved` —— 位没变、语义变了,所以升版本。
//!
//! 计划书 §5.3 原话里还写了 block offset table。**没做**:没有任何调用方要
//! 随机访问某一块,顺序读 + 长度严校验已经能把「Dart 少写了一段」当场判死。
//! 真要随机访问时再加,那时是一次 schema 版本号的事。
//!
//! 同理,`LineMeasure` 只留分页真的用到的四个位。`width` / `baseline` /
//! `hard_break` 在 M1 一个都没用上 —— 按 M0 的教训(「先写字段表必然对不上」),
//! 等 M3 的两端对齐真要 `width` 时再加位、升版本。

/// 行度量的线格式版本。
///
/// **v4**(M3d)也是跟着一起走:这一段的**位一个没动**,变的是页表
/// (摆放记录里 v1 那个 `reserved` 换成了 `line_gap`)。三段共用一个号,
/// 理由见下面 v3 那段。
///
/// **v3**(M2b)只是跟着块表一起走:这一段的**位一个没动**,变的是块表
/// (多了图片那一种,记录里跟着多了 src 区间)。跟着升是因为 Dart 那一半用
/// **同一个常量**校验三段缓冲(`kLayoutSchemaVersion`),而三段本来就是同一次
/// 构建里一起出厂的 —— 分成两个号只会多一处要记得同步的地方,换不来任何
/// 能单独换半边的场景。
pub const MEASURE_SCHEMA_VERSION: u16 = 4;

const HEADER_BYTES: usize = 8;
const BLOCK_HEADER_BYTES: usize = 16;
const LINE_BYTES: usize = 16;

/// 一行的度量。`top` 是**块内**坐标(块顶为 0),不是页内坐标 ——
/// 页内位置由分页器给的 `origin_y` 加上它得到(计划书 §5.4)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMeasure {
    pub render_start: u32,
    pub render_end: u32,
    pub top: f32,
    pub height: f32,
}

/// 一块的度量。`lines` 的下标就是 `Placement::Lines` 引用的行号 ——
/// 两侧必须同序(计划书 §5.3)。
///
/// **图片块也走这里**:它的 render 文本是空的,所以是**一条空行**
/// (`render_start == render_end == 0`),`height` 就是这张图在当前视口里占多高
/// (由 Dart 按图片自身尺寸算,超高的已经压到视口以内)。装页器因此不需要
/// 认识「图片」—— 与 M2a 不让它认识「标题」是同一条:块多一种,装页器不改。
#[derive(Clone, Debug)]
pub struct BlockMeasure {
    pub block_id: u64,
    /// 本块与**前一块**之间的空白。由 Dart 按两块的种类算好(见模块注释),
    /// **页首折叠**:这一块正好排在页顶时不加(不然每页顶上都空一截)。
    pub space_before: f32,
    pub lines: Vec<LineMeasure>,
}

#[derive(Clone, Debug)]
pub struct MeasureBatch {
    pub blocks: Vec<BlockMeasure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// 版本对不上。**整份拒绝**,不按字段猜兼容
    Schema { got: u16, want: u16 },
    /// 缓冲比自描述的长度短(或长):Dart 侧写漏/写多了
    Length { got: usize, want: usize },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Schema { got, want } => write!(f, "行度量线格式版本 {got},期望 {want}"),
            DecodeError::Length { got, want } => {
                write!(f, "行度量缓冲长度 {got} 字节,按自描述应为 {want} 字节")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.buf[self.at..self.at + 4].try_into().expect("4 字节"));
        self.at += 4;
        v
    }

    fn u64(&mut self) -> u64 {
        let v = u64::from_le_bytes(self.buf[self.at..self.at + 8].try_into().expect("8 字节"));
        self.at += 8;
        v
    }

    fn u16(&mut self) -> u16 {
        let v = u16::from_le_bytes(self.buf[self.at..self.at + 2].try_into().expect("2 字节"));
        self.at += 2;
        v
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
}

impl MeasureBatch {
    pub fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < HEADER_BYTES {
            return Err(DecodeError::Length { got: buf.len(), want: HEADER_BYTES });
        }
        let mut r = Reader { buf, at: 0 };
        let version = r.u16();
        if version != MEASURE_SCHEMA_VERSION {
            return Err(DecodeError::Schema { got: version, want: MEASURE_SCHEMA_VERSION });
        }
        let _flags = r.u16();
        let block_count = r.u32() as usize;

        let mut blocks = Vec::with_capacity(block_count.min(4096));
        for _ in 0..block_count {
            if r.at + BLOCK_HEADER_BYTES > buf.len() {
                return Err(DecodeError::Length {
                    got: buf.len(),
                    want: r.at + BLOCK_HEADER_BYTES,
                });
            }
            let block_id = r.u64();
            let line_count = r.u32() as usize;
            let space_before = r.f32();
            let need = r.at + line_count * LINE_BYTES;
            if need > buf.len() {
                return Err(DecodeError::Length { got: buf.len(), want: need });
            }
            let mut lines = Vec::with_capacity(line_count);
            for _ in 0..line_count {
                lines.push(LineMeasure {
                    render_start: r.u32(),
                    render_end: r.u32(),
                    top: r.f32(),
                    height: r.f32(),
                });
            }
            blocks.push(BlockMeasure { block_id, space_before, lines });
        }
        if r.at != buf.len() {
            return Err(DecodeError::Length { got: buf.len(), want: r.at });
        }
        Ok(Self { blocks })
    }

    /// 反向:只给测试与诊断用(产品里写这段的是 Dart)
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&MEASURE_SCHEMA_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(self.blocks.len() as u32).to_le_bytes());
        for b in &self.blocks {
            out.extend_from_slice(&b.block_id.to_le_bytes());
            out.extend_from_slice(&(b.lines.len() as u32).to_le_bytes());
            out.extend_from_slice(&b.space_before.to_bits().to_le_bytes());
            for l in &b.lines {
                out.extend_from_slice(&l.render_start.to_le_bytes());
                out.extend_from_slice(&l.render_end.to_le_bytes());
                out.extend_from_slice(&l.top.to_bits().to_le_bytes());
                out.extend_from_slice(&l.height.to_bits().to_le_bytes());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch() -> MeasureBatch {
        MeasureBatch {
            blocks: vec![
                BlockMeasure {
                    block_id: 0,
                    space_before: 0.0,
                    lines: vec![
                        LineMeasure { render_start: 0, render_end: 10, top: 0.0, height: 32.0 },
                        LineMeasure { render_start: 10, render_end: 18, top: 32.0, height: 32.0 },
                    ],
                },
                BlockMeasure { block_id: 1, space_before: 12.5, lines: vec![] },
            ],
        }
    }

    #[test]
    fn 编解码往返() {
        let b = batch();
        let got = MeasureBatch::decode(&b.encode()).expect("decode");
        assert_eq!(got.blocks.len(), 2);
        assert_eq!(got.blocks[0].lines, b.blocks[0].lines);
        assert!(got.blocks[1].lines.is_empty());
        assert_eq!(got.blocks[1].space_before, 12.5, "段距要原样过一趟线格式");
    }

    #[test]
    fn 版本对不上整份拒绝() {
        let mut buf = batch().encode();
        buf[0] = 9;
        assert_eq!(
            MeasureBatch::decode(&buf).unwrap_err(),
            DecodeError::Schema { got: 9, want: MEASURE_SCHEMA_VERSION }
        );
    }

    #[test]
    fn 少一截当场判死() {
        let buf = batch().encode();
        assert!(matches!(
            MeasureBatch::decode(&buf[..buf.len() - 4]),
            Err(DecodeError::Length { .. })
        ));
    }

    #[test]
    fn 多一截也判死() {
        let mut buf = batch().encode();
        buf.push(0);
        assert!(matches!(MeasureBatch::decode(&buf), Err(DecodeError::Length { .. })));
    }

    #[test]
    fn 空批次合法() {
        let b = MeasureBatch { blocks: vec![] };
        assert!(MeasureBatch::decode(&b.encode()).expect("decode").blocks.is_empty());
    }
}
