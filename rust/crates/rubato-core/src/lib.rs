//! 引擎公共层:实体、错误、以及打破环依赖的 HostEnv 接口。

pub mod cookies;
pub mod entities;
pub mod gson;
pub mod host;
pub mod java_num;
pub mod java_url;
pub mod net_utils;
pub mod rule_data;

pub use host::{
    BookBinding, ElementHandle, HostEnv, JsBindings, JsHost, JsRuleEnv, JsValue, RuleHost,
    SourceBinding, VarStore,
};
pub use java_num::{java_double_to_string, java_float_to_string};
pub use java_url::{JavaUrl, MalformedUrl};
pub use rule_data::{RuleData, VarLayer};
