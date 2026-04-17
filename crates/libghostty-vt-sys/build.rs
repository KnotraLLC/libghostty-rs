use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned ghostty commit. Update this to pull a newer version.
const DEFAULT_GHOSTTY_REPO: &str = "https://github.com/KnotraLLC/ghostty.git";
const DEFAULT_GHOSTTY_COMMIT: &str = "a1e75daef8b64426dbca551c6e41b1fbc2b7ae24";

fn main() {
    // docs.rs has no Zig toolchain. The checked-in bindings in src/bindings.rs
    // are enough for generating documentation, so skip the entire native
    // build when running under docs.rs.
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_NO_VENDOR");
    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_GHOSTTY_REPO");
    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_GHOSTTY_COMMIT");
    println!("cargo:rerun-if-env-changed=GHOSTTY_REPO");
    println!("cargo:rerun-if-env-changed=GHOSTTY_COMMIT");
    println!("cargo:rerun-if-env-changed=GHOSTTY_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-changed=crates/libghostty-vt-sys/build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let target = env::var("TARGET").expect("TARGET must be set");
    let host = env::var("HOST").expect("HOST must be set");
    let ghostty_repo = ghostty_repo();
    let ghostty_commit = ghostty_commit();

    // Resolve Ghostty source in this order:
    // 1. explicit local checkout (fastest path for upstream iteration)
    // 2. pinned vendored fetch (deterministic CI/default path)
    let ghostty_dir = match env::var("GHOSTTY_SOURCE_DIR") {
        Ok(dir) => {
            let p = PathBuf::from(dir);
            assert!(
                p.join("build.zig").exists(),
                "GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
                p.display()
            );
            p
        }
        Err(_) => fetch_ghostty(&out_dir, &ghostty_repo, &ghostty_commit),
    };

    // Build libghostty-vt via zig.
    let install_prefix = out_dir.join("ghostty-install");

    let mut build = Command::new("zig");
    build
        .arg("build")
        .arg("-Demit-lib-vt")
        .arg("--prefix")
        .arg(&install_prefix)
        .current_dir(&ghostty_dir);

    // Only pass -Dtarget when cross-compiling. For native builds, let zig
    // auto-detect the host (matches how ghostty's own CMakeLists.txt works).
    if target != host {
        let zig_target = zig_target(&target);
        build.arg(format!("-Dtarget={zig_target}"));
    }

    run(build, "zig build");

    let lib_dir = install_prefix.join("lib");
    let include_dir = install_prefix.join("include");

    let lib_name = if target.contains("darwin") {
        "libghostty-vt.0.1.0.dylib"
    } else {
        "libghostty-vt.so.0.1.0"
    };

    assert!(
        lib_dir.join(lib_name).exists(),
        "expected shared library at {}",
        lib_dir.join(lib_name).display()
    );
    assert!(
        include_dir.join("ghostty").join("vt.h").exists(),
        "expected header at {}",
        include_dir.join("ghostty").join("vt.h").display()
    );

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=ghostty-vt");
    println!("cargo:include={}", include_dir.display());
}

/// Clone Ghostty at the selected repo+commit into OUT_DIR/ghostty-src.
/// The stamp includes both values so switching forks is deterministic.
fn fetch_ghostty(out_dir: &Path, repo: &str, commit: &str) -> PathBuf {
    let src_dir = out_dir.join("ghostty-src");
    let stamp = src_dir.join(".ghostty-commit");

    // Reuse an existing checkout only when both the source repo and pinned
    // commit match. This avoids accidentally benchmarking the wrong fork just
    // because two repos share a commit object.
    let expected_stamp = format!("{repo}\n{commit}\n");
    if stamp.exists() && let Ok(existing) = std::fs::read_to_string(&stamp) && existing == expected_stamp {
        return src_dir;
    }

    // Clean and clone fresh.
    if src_dir.exists() {
        std::fs::remove_dir_all(&src_dir)
            .unwrap_or_else(|e| panic!("failed to remove {}: {e}", src_dir.display()));
    }

    eprintln!("Fetching ghostty {repo} @ {commit} ...");

    let mut clone = Command::new("git");
    clone
        .arg("clone")
        .arg("--filter=blob:none")
        .arg("--no-checkout")
        .arg(repo)
        .arg(&src_dir);
    run(clone, "git clone ghostty");

    let mut checkout = Command::new("git");
    checkout.arg("checkout").arg(commit).current_dir(&src_dir);
    run(checkout, "git checkout ghostty commit");

    std::fs::write(&stamp, expected_stamp)
        .unwrap_or_else(|e| panic!("failed to write stamp: {e}"));

    src_dir
}

fn ghostty_repo() -> String {
    ghostty_env("LIBGHOSTTY_VT_SYS_GHOSTTY_REPO")
        .or_else(|| ghostty_env("GHOSTTY_REPO"))
        .unwrap_or_else(|| DEFAULT_GHOSTTY_REPO.to_owned())
}

fn ghostty_commit() -> String {
    ghostty_env("LIBGHOSTTY_VT_SYS_GHOSTTY_COMMIT")
        .or_else(|| ghostty_env("GHOSTTY_COMMIT"))
        .unwrap_or_else(|| DEFAULT_GHOSTTY_COMMIT.to_owned())
}

fn ghostty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn run(mut command: Command, context: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {context}: {error}"));
    assert!(status.success(), "{context} failed with status {status}");
}

fn zig_target(target: &str) -> String {
    let value = match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "aarch64-apple-darwin" => "aarch64-macos-none",
        "x86_64-apple-darwin" => "x86_64-macos-none",
        other => panic!("unsupported Rust target for vendored build: {other}"),
    };
    value.to_owned()
}
