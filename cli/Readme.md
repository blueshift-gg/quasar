# Cli

## Profiler
### Command to run profiler
CARGO_PROFILE_RELEASE_DEBUG=2 cargo build-sbf --manifest-path examples/escrow/Cargo.toml --lto && \
cargo run --release -p quasar-profile -- target/sbpf-solana-solana/release/quasar_escrow.so

#### Default behavior
- writes <`program`>.profile.json
- publishes a private gist and prints the URL to open the profile in the Fuego profiler UI

#### Optional modes
cargo run --release -p quasar-profile -- target/sbpf-solana-solana/release/quasar_escrow.so --no-gist
cargo run --release -p quasar-profile -- target/sbpf-solana-solana/release/quasar_escrow.so --share
cargo run --release -p quasar-profile -- target/sbpf-solana-solana/release/quasar_escrow.so --folded
cargo run --release -p quasar-profile -- target/sbpf-solana-solana/release/quasar_escrow.so --text
