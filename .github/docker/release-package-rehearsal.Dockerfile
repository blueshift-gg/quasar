FROM rust:1.92.0-slim-trixie@sha256:bf3368a992915f128293ac76917ab6e561e4dda883273c8f5c9f6f8ea37a378e AS base

ARG DEBIAN_SNAPSHOT=20260113T000000Z

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
    jq \
    libssl-dev \
    pkg-config \
    procps \
    python3 \
    && rm -rf /var/lib/apt/lists/*

FROM base AS packager

ENV CARGO_TERM_COLOR=always
WORKDIR /workspace/quasar
COPY . /workspace/quasar

RUN make PACKAGE_REHEARSAL_ROOT=/opt/quasar-release-rehearsal package-rehearsal \
    && cargo --config /opt/quasar-release-rehearsal/cargo-config.toml \
        install \
        --path /opt/quasar-release-rehearsal/packages/quasar-cli-0.1.0 \
        --root /opt/quasar-cli \
        --locked \
    && /opt/quasar-cli/bin/quasar --version

FROM base AS rehearsal

ARG SOLANA_VERSION=v4.1.1
ARG SOLANA_LINUX_SHA256

ENV CARGO_HOME=/home/quasar/.cargo
ENV CARGO_TERM_COLOR=always
ENV HOME=/home/quasar
ENV PATH="/opt/quasar-cli/bin:/opt/solana/active_release/bin:/usr/local/cargo/bin:${PATH}"

COPY scripts/install-solana-tools.sh /usr/local/bin/install-solana-tools

RUN HOME=/root XDG_CACHE_HOME=/root/.cache \
        install-solana-tools "${SOLANA_VERSION}" "${SOLANA_LINUX_SHA256}" /opt/solana \
    && cargo-build-sbf --version \
    && rm -rf /root/.cache/quasar/solana

COPY --from=packager /opt/quasar-cli /opt/quasar-cli
COPY --from=packager /opt/quasar-release-rehearsal /opt/quasar-release-rehearsal
COPY .github/scripts/release-package-rehearsal.sh /usr/local/bin/quasar-release-package-rehearsal

RUN quasar --version \
    && test "$(find /opt/quasar-release-rehearsal/archives -type f -name '*.crate' | wc -l)" -eq 10 \
    && test "$(find /opt/quasar-release-rehearsal/packages -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 10 \
    && groupadd --gid 10001 quasar \
    && useradd --uid 10001 --gid quasar --create-home --shell /bin/bash quasar \
    && install -d -o quasar -g quasar /home/quasar/.cargo /rehearsal/projects \
    && install -d /rehearsal/.cargo \
    && cp /opt/quasar-release-rehearsal/cargo-config.toml /rehearsal/.cargo/config.toml \
    && find /opt/quasar-release-rehearsal -type f -exec chmod 0444 {} + \
    && find /opt/quasar-release-rehearsal -type d -exec chmod 0555 {} + \
    && chmod 0444 /rehearsal/.cargo/config.toml \
    && chmod 0555 /rehearsal/.cargo \
    && rm -rf /usr/local/cargo/registry/cache \
        /usr/local/cargo/registry/index \
        /usr/local/cargo/registry/src \
        /usr/local/cargo/git

USER quasar
WORKDIR /rehearsal/projects
ENTRYPOINT ["/usr/local/bin/quasar-release-package-rehearsal"]
