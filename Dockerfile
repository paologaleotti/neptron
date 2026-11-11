# The NEPTRON development container: the host side of a Buildroot build.
#
# Buildroot needs Linux and a case-sensitive filesystem, so on macOS the
# images under os/ build inside this container. QEMU runs on the host on
# macOS, and can run in this container on Linux CI.

FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

# The packages that the Buildroot manual lists as mandatory, plus ncurses for
# menuconfig and QEMU for the emulated targets on Linux hosts.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bc \
        bzip2 \
        ca-certificates \
        cpio \
        file \
        g++ \
        gcc \
        git \
        gzip \
        libncurses-dev \
        make \
        patch \
        perl \
        python3 \
        qemu-system-arm \
        qemu-system-x86 \
        rsync \
        tar \
        unzip \
        wget \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Buildroot is fetched at a pinned release and checked against the SHA-256 in
# the signed release announcement. It is never copied into the repository.
ARG BUILDROOT_VERSION=2026.08.1
ARG BUILDROOT_SHA256=38a6c152490096018720a9ade379ee1e571c9bf5f93a3657a166566945a98b0d
RUN wget -q -O /tmp/buildroot.tar.gz \
        "https://buildroot.org/downloads/buildroot-${BUILDROOT_VERSION}.tar.gz" \
    && echo "${BUILDROOT_SHA256}  /tmp/buildroot.tar.gz" | sha256sum -c - \
    && mkdir -p /opt/buildroot \
    && tar -xzf /tmp/buildroot.tar.gz -C /opt/buildroot --strip-components=1 \
    && rm /tmp/buildroot.tar.gz

# Buildroot builds as a normal user. The work directories exist in the image
# with this owner, so that a new named volume mounted on them gets it too.
RUN useradd --create-home --uid 1000 br \
    && mkdir -p /work/out /work/dl /work/images /home/br/.buildroot-ccache \
    && chown -R br:br /work /home/br/.buildroot-ccache

ENV LANG=C.UTF-8 \
    BR2_DL_DIR=/work/dl

USER br
WORKDIR /work
