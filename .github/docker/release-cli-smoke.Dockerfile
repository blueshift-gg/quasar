FROM rust:1.92.0-slim-trixie@sha256:bf3368a992915f128293ac76917ab6e561e4dda883273c8f5c9f6f8ea37a378e

ARG SOLANA_VERSION=v4.1.1
ARG SOLANA_LINUX_SHA256
ARG DEBIAN_SNAPSHOT=20260113T000000Z

ENV CARGO_TERM_COLOR=always
ENV PATH="/opt/quasar-cli/bin:/root/.local/share/solana/install/active_release/bin:${PATH}"
ENV QUASAR_SOURCE=/workspace/quasar

RUN sed -ri \
    -e "s#https?://deb.debian.org/debian-security#https://snapshot.debian.org/archive/debian-security/${DEBIAN_SNAPSHOT}#" \
    -e "s#https?://deb.debian.org/debian#https://snapshot.debian.org/archive/debian/${DEBIAN_SNAPSHOT}#" \
    /etc/apt/sources.list.d/debian.sources \
    && printf 'Acquire::Check-Valid-Until "false";\n' > /etc/apt/apt.conf.d/99snapshot \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    bash \
    bzip2 \
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

RUN scripts/install-solana-tools.sh "${SOLANA_VERSION}" "${SOLANA_LINUX_SHA256}" \
    && cargo install --path cli --root /opt/quasar-cli --locked \
    && cargo build-sbf --version \
    && quasar --version \
    && rm -rf target /root/.cache/quasar/solana \
        /usr/local/cargo/registry/cache /usr/local/cargo/git/checkouts

WORKDIR /workspace
ENTRYPOINT ["/workspace/quasar/.github/scripts/release-cli-smoke.sh"]
