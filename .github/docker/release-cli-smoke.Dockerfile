FROM rust:1.92.0-slim-trixie

ARG SOLANA_VERSION=v4.1.1

ENV CARGO_TERM_COLOR=always
ENV PATH="/opt/quasar-cli/bin:/root/.local/share/solana/install/active_release/bin:${PATH}"
ENV QUASAR_SOURCE=/workspace/quasar

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    curl \
    git \
    libssl-dev \
    pkg-config \
    python3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace/quasar
COPY . /workspace/quasar

RUN sh -c "$(curl -sSfL https://release.anza.xyz/${SOLANA_VERSION}/install)" \
    && cargo install --path cli --root /opt/quasar-cli \
    && cargo build-sbf --version \
    && quasar --version \
    && rm -rf target /usr/local/cargo/registry/cache /usr/local/cargo/git/checkouts

WORKDIR /workspace
ENTRYPOINT ["/workspace/quasar/.github/scripts/release-cli-smoke.sh"]
