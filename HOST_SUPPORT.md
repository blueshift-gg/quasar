# Quasar v0.1.0 host support

Quasar v0.1.0 supports the following native development hosts:

| Operating system | Architecture | Rust host triple | Required release CI runner |
|---|---|---|---|
| Ubuntu 24.04 LTS | x86-64 | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` |
| macOS 15 | Apple Silicon | `aarch64-apple-darwin` | `macos-15` |
| macOS 15 | Intel | `x86_64-apple-darwin` | `macos-15-intel` |

Release CI must compile all ten publishable crates, including `quasar-cli`, on
each row. The Ubuntu row also runs the SBF and host integration suites. The
macOS rows cover package and CLI compilation.

## Windows

Use WSL2 with Ubuntu 24.04 x86-64 on an x86-64 Windows machine. Quasar treats
that environment as the supported Ubuntu row above.

Quasar v0.1.0 does not support the CLI on native Windows through PowerShell,
Command Prompt, or MSYS2. The release does not test native Windows paths or
claim that commands which discover Solana tools and executables work there.

## Other hosts

Quasar may compile on other systems, but v0.1.0 makes no support claim for
them. This includes Linux on Arm, Linux distributions or libc variants other
than Ubuntu 24.04 with glibc, other macOS releases, and 32-bit hosts.

Quasar adds a host to this matrix only when the pinned
[Agave v4.1.1](https://github.com/anza-xyz/agave/releases/tag/v4.1.1) and
[platform-tools v1.52](https://github.com/anza-xyz/platform-tools/releases/tag/v1.52)
releases ship matching binaries and Quasar runs a required release CI row for
it. Adding another host requires its own compatibility review and CI row.
