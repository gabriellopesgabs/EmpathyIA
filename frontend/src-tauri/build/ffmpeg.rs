// FFmpeg is a release artifact, not an implicit network dependency.
//
// Release jobs must provision a reviewed FFmpeg binary through
// EMPATHY_FFMPEG_PATH. Native development builds may use a system FFmpeg.
// The build never downloads executable code from a third-party release.

use std::path::{Path, PathBuf};

pub fn ensure_ffmpeg_binary() {
    let target = std::env::var("TARGET")
        .or_else(|_| std::env::var("HOST"))
        .expect("Neither TARGET nor HOST environment variable is set");
    let host = std::env::var("HOST").unwrap_or_else(|_| target.clone());
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));
    let binaries_dir = manifest_dir.join("binaries");
    let binary_name = if target.contains("windows") {
        format!("ffmpeg-{}.exe", target)
    } else {
        format!("ffmpeg-{}", target)
    };
    let destination = binaries_dir.join(binary_name);

    println!("cargo:rerun-if-env-changed=EMPATHY_FFMPEG_PATH");
    println!("cargo:rerun-if-changed={}", destination.display());

    if destination.exists() && verify_ffmpeg_binary(&destination) {
        println!(
            "cargo:warning=Using provisioned FFmpeg sidecar: {}",
            destination.display()
        );
        return;
    }

    let source = std::env::var_os("EMPATHY_FFMPEG_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            if host == target {
                which::which(if target.contains("windows") {
                    "ffmpeg.exe"
                } else {
                    "ffmpeg"
                })
                .ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            panic!(
                "FFmpeg sidecar is missing for {target}. Set EMPATHY_FFMPEG_PATH to a reviewed binary. EmpathyIA intentionally does not download executables during the build."
            )
        });

    if !verify_ffmpeg_binary(&source) {
        panic!("FFmpeg verification failed for {}", source.display());
    }

    std::fs::create_dir_all(&binaries_dir).expect("Failed to create binaries directory");
    std::fs::copy(&source, &destination).unwrap_or_else(|error| {
        panic!(
            "Failed to copy FFmpeg from {} to {}: {}",
            source.display(),
            destination.display(),
            error
        )
    });

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&destination)
            .expect("Failed to inspect copied FFmpeg")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&destination, permissions)
            .expect("Failed to make FFmpeg executable");
    }
}

fn verify_ffmpeg_binary(path: &Path) -> bool {
    std::process::Command::new(path)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
