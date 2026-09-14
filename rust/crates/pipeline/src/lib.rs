//! WebBook 四步流水线(搜索/发现/详情/目录/正文)。
//! 差分:tools/pipeline_diff.sh;契约 fixtures/cases/pipeline/README.md。

pub mod helpers;
// HtmlFormatter 下沉成了共用 crate(js-host 的 `java.htmlFormat` 也要它),
// 这里保留原来的路径名,调用点不动。
pub use html_format as html_formatter;
/// 发现页分类列表(`BookSourceExtensions.exploreKinds()`)
pub mod explore_kinds;
pub mod web_book;

pub use web_book::{
    PipelineEnv, PipelineError, explore_book, explore_fetch, get_book_info, get_chapter_list,
    get_content, get_image, search_book, search_fetch,
};
