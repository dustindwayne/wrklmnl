"""Generate notices and source companions for a locked WRKLMNL build."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
WINDOWS = "x86_64-pc-windows-msvc"
SUPPLEMENTS = ROOT / "licenses" / "upstream"
NOTICE_NAME = "THIRD_PARTY_NOTICES.txt"
LICENSE_FILE = re.compile(
    r"^(licen[cs]e|copying|notice|copyright|ofl|ufl|unlicense)(\b|[-_.])",
    re.IGNORECASE,
)
REVIEWED = {
    "0BSD", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "BSL-1.0", "ISC",
    "MIT", "MPL-2.0", "OFL-1.1", "Ubuntu-font-1.0", "Unicode-3.0", "Unlicense", "Zlib",
}


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, encoding="utf-8").strip()


def packages(target, offline):
    flags = ["--locked", *(["--offline"] if offline else [])]
    tree = command(
        "cargo", "tree", "-p", "dterm", "--target", target, "--edges",
        "normal,build", "--prefix", "none", "--format", "{p}", *flags,
    )
    used = set(re.findall(r"^([^\s]+) v([^\s]+)", tree, re.MULTILINE))
    metadata = json.loads(command(
        "cargo", "metadata", "--format-version", "1", "--filter-platform", target, *flags,
    ))
    return sorted(
        (package for package in metadata["packages"]
         if package["source"] and (package["name"], package["version"]) in used),
        key=lambda package: (package["name"], package["version"]),
    )


def generate(target, dependency_packages):
    overrides = json.loads((SUPPLEMENTS / "sources.json").read_text(encoding="utf-8"))
    texts = {}
    inventory = []
    mpl = []

    def record(label, path):
        text = path.read_text(encoding="utf-8-sig").strip()
        item = texts.setdefault(text, (len(texts) + 1, []))
        item[1].append(label)
        return f"text {item[0]}"

    for package in dependency_packages:
        name, version = package["name"], package["version"]
        identifier = f"{name} {version}"
        expression = package["license"] or ""
        terms = set(re.findall(r"[A-Za-z0-9][A-Za-z0-9.-]*", expression)) - {"AND", "OR"}
        if not terms or terms - REVIEWED:
            raise SystemExit(f"Review the license for {identifier}: {expression or '(undeclared)'}")
        if "MPL-2.0" in terms:
            mpl.append(package)

        folder = Path(package["manifest_path"]).parent
        files = [
            file for file in folder.rglob("*")
            if file.is_file() and LICENSE_FILE.match(file.name)
        ]
        if name == "epaint_default_fonts":
            files.extend(folder / "fonts" / font for font in (
                "Hack-Regular.txt", "emoji-icon-font-mit-license.txt",
            ))
        if package["license_file"]:
            files.append(folder / package["license_file"])
        links = []
        for file in sorted(set(files), key=lambda path: path.relative_to(folder).as_posix()):
            links.append(record(f"{identifier} / {file.relative_to(folder).as_posix()}", file))
        for item in overrides.get(f"{name}@{version}", []):
            links.append(record(f"{identifier} / {item['source']}", SUPPLEMENTS / item["file"]))
        if not links:
            raise SystemExit(f"Supply upstream license text for {identifier}")
        inventory.append(f"{identifier} — {expression} ({', '.join(dict.fromkeys(links))})")

    inventory.extend((
        "Lucide and Feather icon geometry — ISC and MIT ("
        + record("Lucide and Feather icon geometry", ROOT / "assets/icons/LICENSE") + ")",
        "Vendored eframe 0.31.1 — distributed under MIT ("
        + record("Vendored eframe 0.31.1", ROOT / "vendor/eframe/LICENSE-MIT") + ")",
    ))
    version = command("rustc", "--version")
    lines = [
        "WRKLMNL THIRD-PARTY NOTICES",
        f"Build target: {target}",
        f"Toolchain: {version}",
        "",
        "These terms apply to the third-party components named below. The project",
        "has no project-wide open-source license. The inventory includes the locked",
        "runtime dependencies and build dependencies that may contribute generated code.",
        "",
        "Rust standard library: see the accompanying RUST-STDLIB-NOTICES.html.",
        "The original document is also installed as share/doc/rust/COPYRIGHT-library.html",
        "by the rust-docs component of the Rust toolchain identified above.",
        "",
        "Mozilla Public License 2.0 — corresponding source",
        "The MPL-covered components below are shipped without modifications.",
        "The corresponding Source Code Form is available under MPL-2.0, without charge,",
        "in the accompanying .crate files or from these exact upstream archives:",
    ]
    for package in mpl:
        name, version = package["name"], package["version"]
        lines.append(f"  {name} {version}: https://static.crates.io/crates/{name}/{name}-{version}.crate")
    lines.extend((
        "",
        "Default font files in epaint_default_fonts are redistributed unmodified as",
        "part of the app. Their notices include Noto Emoji, Ubuntu Light, Hack with",
        "Bitstream Vera terms, and emoji-icon-font. Optional system and user fonts",
        "are read locally and are outside this distribution.",
        "",
        "Component inventory",
        *inventory,
        "",
        "License and attribution texts",
    ))
    for body, (number, labels) in texts.items():
        lines.extend(("", f"----- text {number} -----", *labels, "", body))
    return "\n".join(lines) + "\n", mpl


def bundle(folder, notices, mpl):
    folder.mkdir(parents=True, exist_ok=True)
    sysroot = Path(command("rustc", "--print", "sysroot"))
    rust_notices = sysroot / "share/doc/rust/COPYRIGHT-library.html"
    if not rust_notices.is_file():
        raise SystemExit("Install rust-docs for this toolchain before bundling notices.")
    shutil.copyfile(rust_notices, folder / "RUST-STDLIB-NOTICES.html")
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    checksums = {
        (entry["name"], entry["version"]): entry.get("checksum")
        for entry in lock["package"]
    }
    for package in mpl:
        name, version = package["name"], package["version"]
        manifest = Path(package["manifest_path"])
        registry = manifest.parents[3]
        index = manifest.parents[1].name
        archive = f"{name}-{version}.crate"
        source = registry / "cache" / index / archive
        expected = checksums[name, version]
        if not expected or hashlib.sha256(source.read_bytes()).hexdigest() != expected:
            raise SystemExit(f"{archive} does not match Cargo.lock")
        shutil.copyfile(source, folder / archive)
    (folder / NOTICE_NAME).write_text(notices, encoding="utf-8", newline="\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default=WINDOWS)
    parser.add_argument("--output", type=Path, default=ROOT / NOTICE_NAME)
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--offline", action="store_true")
    options = parser.parse_args()
    dependency_packages = packages(options.target, options.offline)
    notices, mpl = generate(options.target, dependency_packages)
    if options.check:
        if options.output.read_text(encoding="utf-8") != notices:
            raise SystemExit("Third-party notices are stale. Review and regenerate the notice file.")
    else:
        options.output.write_text(notices, encoding="utf-8", newline="\n")
    if options.bundle:
        bundle(options.bundle, notices, mpl)
    print(f"Verified notices for {len(dependency_packages)} registry packages, {len(mpl)} MPL sources.")


if __name__ == "__main__":
    main()
