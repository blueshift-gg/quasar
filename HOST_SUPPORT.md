# Quasar v0.1.0 host support

Quasar v0.1.0 supports these native development hosts:

| Operating system | Architecture | Rust host triple | Release runner |
| --- | --- | --- | --- |
| Ubuntu 24.04 LTS | x86-64 | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` |
| macOS 15 | Apple Silicon | `aarch64-apple-darwin` | `macos-15` |
| macOS 15 | Intel | `x86_64-apple-darwin` | `macos-15-intel` |

Release CI compiles the Cargo-metadata-derived publishable graph, including the
CLI, on each row. Ubuntu also owns the SBF and package-journey gates. The macOS
rows protect package and CLI compilation without maintaining a second package
inventory.

## Required toolchain

Quasar uses the supported Solana `cargo build-sbf` toolchain. For 0.1.0 that
means Agave v4.1.1 and platform-tools v1.52. Init, build, lint, and package
rehearsal exercise this path.

## Windows

On an x86-64 Windows host, use WSL2 with Ubuntu 24.04 x86-64. Quasar treats
that as the Ubuntu environment above.

Native Windows through PowerShell, Command Prompt, or MSYS2 is unsupported.
Quasar does not claim that Solana executable discovery or program build and
deployment work on native Windows.

## Other hosts

Quasar may compile elsewhere, but v0.1.0 makes no support claim for Linux on
Arm, other Linux distributions or libc variants, other macOS releases, or
32-bit hosts.

A new host becomes supported only when the pinned Solana toolchain ships the
required binaries and Quasar runs the appropriate release journey on a
required CI row.
