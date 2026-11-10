use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    println!("cargo:rerun-if-changed=assets/wrklmnl.ico");
    println!("cargo:rerun-if-env-changed=WindowsSdkVerBinPath");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::copy("assets/wrklmnl.ico", out.join("wrklmnl.ico")).expect("stage application icon");
    fs::write(out.join("branding.rc"), "1 ICON \"wrklmnl.ico\"\n")
        .expect("write icon resource script");

    let status = Command::new(resource_compiler())
        .current_dir(&out)
        .args(["/nologo", "/fo", "branding.res", "branding.rc"])
        .status()
        .expect("run rc.exe from the Windows SDK");
    assert!(status.success(), "compile WRKLMNL Windows icon");
    println!(
        "cargo:rustc-link-arg-bins={}",
        out.join("branding.res").display()
    );
}

fn resource_compiler() -> PathBuf {
    let arch = match env::var("HOST").unwrap_or_default().split('-').next() {
        Some("aarch64") => "arm64",
        Some("i686") => "x86",
        _ => "x64",
    };
    if let Some(sdk) = env::var_os("WindowsSdkVerBinPath") {
        let path = PathBuf::from(sdk).join(arch).join("rc.exe");
        if path.is_file() {
            return path;
        }
    }
    if let Some(program_files) = env::var_os("ProgramFiles(x86)") {
        let bin = PathBuf::from(program_files).join("Windows Kits/10/bin");
        if let Ok(versions) = fs::read_dir(bin) {
            if let Some(path) = versions
                .flatten()
                .map(|version| version.path().join(arch).join("rc.exe"))
                .filter(|path| path.is_file())
                .max()
            {
                return path;
            }
        }
    }
    PathBuf::from("rc.exe")
}
