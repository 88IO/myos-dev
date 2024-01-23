FROM mcr.microsoft.com/vscode/devcontainers/base:ubuntu-22.04

ARG USERNAME=vscode

RUN apt-get update \
 && apt-get upgrade -y

RUN apt-get install -y --no-install-recommends \
    build-essential \
    llvm-14-dev \
    lld-14 \
    clang-14 \
    nasm \
    acpica-tools \
    uuid-dev \
    qemu-system-x86 \
    qemu-utils \
    xauth \
    unzip \
    qemu-system-gui \
    python3-distutils \
    ca-certificates \
    dosfstools \
    git \
    curl \
    sudo \
 && apt-get clean -y \
 && rm -rf /var/lib/apt/lists

RUN for item in \
        llvm-PerfectShuffle \
        llvm-ar \
        llvm-as \
        llvm-bcanalyzer \
        llvm-cat \
        llvm-cfi-verify \
        llvm-config \
        llvm-cov \
        llvm-c-test \
        llvm-cvtres \
        llvm-cxxdump \
        llvm-cxxfilt \
        llvm-diff \
        llvm-dis \
        llvm-dlltool \
        llvm-dwarfdump \
        llvm-dwp \
        llvm-exegesis \
        llvm-extract \
        llvm-lib \
        llvm-link \
        llvm-lto \
        llvm-lto2 \
        llvm-mc \
        llvm-mca \
        llvm-modextract \
        llvm-mt \
        llvm-nm \
        llvm-objcopy \
        llvm-objdump \
        llvm-opt-report \
        llvm-pdbutil \
        llvm-profdata \
        llvm-ranlib \
        llvm-rc \
        llvm-readelf \
        llvm-readobj \
        llvm-rtdyld \
        llvm-size \
        llvm-split \
        llvm-stress \
        llvm-strings \
        llvm-strip \
        llvm-symbolizer \
        llvm-tblgen \
        llvm-undname \
        llvm-xray \
        ld.lld \
        lld-link \
        clang \
        clang++ \
        clang-cpp \
    ; do \
        update-alternatives --install "/usr/bin/${item}" "${item}" "/usr/bin/${item}-14" 50 \
    ; done

RUN ln -sf /home/${USERNAME}/edk2 /
RUN ln -sf /home/${USERNAME}/osbook /

USER ${USERNAME}
WORKDIR /home/${USERNAME}

RUN git clone -b edk2-stable202208 --recursive https://github.com/tianocore/edk2.git edk2 \
 && make -C edk2/BaseTools/Source/C

RUN git clone https://github.com/uchan-nos/mikanos-build.git osbook

RUN curl -L https://github.com/uchan-nos/mikanos-build/releases/download/v2.0/x86_64-elf.tar.gz \
  | tar xzvf - -C osbook/devenv

ENV PATH="/home/${USERNAME}/.cargo/bin:/home/${USERNAME}/osbook/devenv:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    rustup default nightly && \
    rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu && \
    rustup target add x86_64-unknown-uefi && \
    cargo install cargo-binutils && \
    cargo install cargo-make

RUN ln -s /workspace/mikanos/MikanLoaderPkg edk2/

COPY ./setup.sh /tmp/
RUN cat /tmp/setup.sh >> .bashrc