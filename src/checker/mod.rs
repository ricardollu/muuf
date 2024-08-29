const VIDEO_EXTS: [&str; 2] = ["mp4", "mkv"];

mod collection;
mod mikan;
mod res_rule;

pub use collection::check_collection;
pub use mikan::check_mikan;
pub use res_rule::check_res_rule;
