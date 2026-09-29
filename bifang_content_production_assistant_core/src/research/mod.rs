//! 对标采集、风格资产与原创内容生成的接口无关业务规则。

mod model;
mod generation;
mod library;
mod delivery;

pub use model::*;
pub use generation::*;
pub use library::*;
pub use delivery::*;

#[cfg(test)]
mod tests;
