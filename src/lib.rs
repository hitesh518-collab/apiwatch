#![doc = "Internal APIWatch library. Public interfaces are pre-v1 and unstable."]

pub fn version_string() -> &'static str {
    let ver = env!("CARGO_PKG_VERSION");
    let s = match option_env!("GIT_HASH") {
        Some("") | None => ver.to_owned(),
        Some(hash) => format!("{ver} ({hash})"),
    };
    Box::leak(s.into_boxed_str())
}

#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod config;
#[doc(hidden)]
pub mod contract;
#[doc(hidden)]
pub mod diff;
#[doc(hidden)]
pub mod har;
#[doc(hidden)]
pub mod lock_size;
#[doc(hidden)]
pub mod lockfile;
#[doc(hidden)]
pub mod observed;
#[doc(hidden)]
pub mod openapi;
#[doc(hidden)]
pub mod output;
#[doc(hidden)]
pub mod remote;
