# Distribution

The GitHub workflow builds Windows x64. The same checkout has native source-build paths for Linux and macOS.

## Build from source

### Windows

Use the Rust MSVC toolchain, Visual Studio C++ Build Tools, and a Windows SDK. The SDK supplies the resource compiler for the launcher icon. The [Windows workflow](../.github/workflows/windows.yml) tests and builds the x64 release executable using Rust 1.99.1.

```powershell
cargo test --locked --workspace
cargo build --locked --release --bin WRKLMNL
.\target\release\WRKLMNL.exe
```

### Linux

The x86-64 source build was verified previously on Ubuntu with Rust 1.91.1. Install a C compiler, `pkg-config`, and the libudev development headers. Run the app from an X11 or Wayland graphical session with OpenGL support. Under WSL, use the Linux filesystem for file-move tests.

```bash
cargo test --locked --workspace
cargo build --locked --release --bin WRKLMNL
./target/release/WRKLMNL
```

### macOS

Install the Xcode Command Line Tools and a native Rust toolchain for Apple Silicon or Intel. Run the same Cargo build from a macOS graphical session. The command produces a native executable in `target/release`. The macOS build is unverified.

```bash
cargo test --locked --workspace
cargo build --locked --release --bin WRKLMNL
./target/release/WRKLMNL
```

## Notices and source

The Windows release ships as `WRKLMNL.exe`. **Settings → Licenses** provides the embedded third-party and Rust standard library notices. The repository also contains the [third-party notices](../THIRD_PARTY_NOTICES.txt) and [compressed Rust standard library notices](../licenses/rust-stdlib-notices.html.gz).

The exact, unmodified source archives for [serialport 4.10.1](https://static.crates.io/crates/serialport/serialport-4.10.1.crate) and [option-ext 0.2.0](https://static.crates.io/crates/option-ext/option-ext-0.2.0.crate) are available under [MPL-2.0](https://www.mozilla.org/en-US/MPL/2.0/). WRKLMNL’s own code has no project-wide open-source license.

## Reproduce the notices

Use Python 3.11 or later and the same Rust toolchain as the executable. Install the toolchain’s `rust-docs` component so its standard library notices are available.

```powershell
rustup component add rust-docs --toolchain 1.99.1
$env:RUSTUP_TOOLCHAIN = '1.99.1'
python scripts/third_party_notices.py --check
```

For a dependency or toolchain update, review the license changes and regenerate the embedded notices by running the same script without `--check`. The script collects the locked runtime and build dependencies and stops on unreviewed license identifiers or missing texts. The [supplement index](../licenses/upstream/sources.json) records license texts omitted from their registry packages.

For a Linux or macOS distribution, run the script on the native build host. Pass the Rust host triple as `--target`, set `--output` to a platform-specific notice file, and use `--bundle` to stage the accompanying files. Review any newly resolved dependencies and provide their upstream texts before distributing. The checked-in notices describe Windows x64.

```bash
host=$(rustc -vV | sed -n 's/^host: //p')
python3 scripts/third_party_notices.py --target "$host" --output "THIRD_PARTY_NOTICES-$host.txt" --bundle "dist/$host"
```
