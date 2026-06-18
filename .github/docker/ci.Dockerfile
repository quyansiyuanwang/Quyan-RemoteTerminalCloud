FROM ubuntu:22.04

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        libwebkit2gtk-4.1-dev \
        libayatana-appindicator3-dev \
        librsvg2-dev \
        patchelf \
        && rm -rf /var/lib/apt/lists/*

# Default to a non-root user matching GitHub runner uid
RUN useradd -m -u 1001 runner
USER runner
