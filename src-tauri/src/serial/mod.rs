#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod desktop;
#[cfg(any(target_os = "ios", target_os = "android"))]
mod ios;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use desktop::*;
#[cfg(any(target_os = "ios", target_os = "android"))]
pub use ios::*;
