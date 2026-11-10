# Distribution

The GitHub workflow builds Windows x64. Linux and macOS can build natively from the same checkout using the instructions in the README.

## Windows release files

| File | Purpose |
| --- | --- |
| `WRKLMNL.exe` | Optimized desktop executable |
| `THIRD_PARTY_NOTICES.txt` | Windows dependency inventory, license texts, icon and font notices, and source links |
| `RUST-STDLIB-NOTICES.html` | Standard library copyright notices shipped with the release toolchain |
| `serialport-4.10.1.crate` and `option-ext-0.2.0.crate` | Exact, unmodified MPL-2.0 source archives from the locked build |

Keep these files together on the release page. The MPL covers the named dependency files. WRKLMNL’s own code has no project-wide open-source license.

## Reproduce the notices

Use Python 3.11 or later and the same Rust toolchain as the executable. Install the toolchain’s `rust-docs` component so its standard library notices are available.

```powershell
rustup component add rust-docs --toolchain 1.99.1
$env:RUSTUP_TOOLCHAIN = '1.99.1'
python scripts/third_party_notices.py --check --bundle dist
```

For a dependency or toolchain update, review the license changes and regenerate `THIRD_PARTY_NOTICES.txt` by running the same script without `--check`. The script collects the locked runtime and build dependencies. It stops on unreviewed license identifiers or missing texts and verifies MPL source archives against `Cargo.lock`. The [supplement index](../licenses/upstream/sources.json) records license texts omitted from their registry packages.

For a Linux or macOS distribution, run the script on the native build host. Pass the Rust host triple as `--target`, set `--output` to a platform-specific notice file, and use `--bundle` to stage the accompanying files. Review any newly resolved dependencies and provide their upstream texts before distributing. The checked-in notices describe Windows x64.

```bash
host=$(rustc -vV | sed -n 's/^host: //p')
python3 scripts/third_party_notices.py --target "$host" --output "THIRD_PARTY_NOTICES-$host.txt" --bundle "dist/$host"
```
