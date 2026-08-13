#[cfg(windows)]
pub mod windows;
#[cfg(not(windows))]
compile_error!("rmod only supports Windows");