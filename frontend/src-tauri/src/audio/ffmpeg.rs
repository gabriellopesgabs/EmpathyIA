use log::debug;
use once_cell::sync::Lazy;
use std::path::PathBuf;
use which::which;

#[cfg(not(windows))]
const EXECUTABLE_NAME: &str = "ffmpeg";
#[cfg(windows)]
const EXECUTABLE_NAME: &str = "ffmpeg.exe";

static FFMPEG_PATH: Lazy<Option<PathBuf>> = Lazy::new(find_ffmpeg_path_internal);

pub fn find_ffmpeg_path() -> Option<PathBuf> {
    FFMPEG_PATH.clone()
}

fn is_executable_file(path: &PathBuf) -> bool {
    path.is_file()
}

fn find_ffmpeg_path_internal() -> Option<PathBuf> {
    debug!("Searching for a trusted, pre-provisioned FFmpeg binary");

    if let Some(path) = std::env::var_os("EMPATHY_FFMPEG_PATH").map(PathBuf::from) {
        if is_executable_file(&path) {
            return Some(path);
        }
        log::warn!(
            "EMPATHY_FFMPEG_PATH does not point to a file: {}",
            path.display()
        );
    }

    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            for candidate in [
                directory.join(EXECUTABLE_NAME),
                directory.join("../Resources").join(EXECUTABLE_NAME),
                directory.join("lib").join(EXECUTABLE_NAME),
            ] {
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }

    if let Ok(path) = which(EXECUTABLE_NAME) {
        return Some(path);
    }

    // Source builds use the reviewed sidecar provisioned by build/ffmpeg.rs.
    let target_binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("ffmpeg-{}{}", target_triple(), executable_suffix()));
    if is_executable_file(&target_binary) {
        return Some(target_binary);
    }

    log::error!(
        "FFmpeg is unavailable. Empathy will not download or execute an unverified binary at runtime."
    );
    None
}

fn executable_suffix() -> &'static str {
    if cfg!(windows) {
        ".exe"
    } else {
        ""
    }
}

fn target_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return "aarch64-apple-darwin";
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return "x86_64-apple-darwin";
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return "x86_64-pc-windows-msvc";
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return "x86_64-unknown-linux-gnu";
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return "aarch64-unknown-linux-gnu";
    }
    #[allow(unreachable_code)]
    "unsupported-target"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_binary_name_matches_target() {
        assert!(!target_triple().is_empty());
        assert_eq!(executable_suffix(), if cfg!(windows) { ".exe" } else { "" });
    }
}
