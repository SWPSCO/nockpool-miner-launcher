FROM rustlang/rust:nightly-bookworm AS builder
RUN apt update && apt install git
RUN rustup target add x86_64-unknown-linux-gnu
WORKDIR /build
COPY . .
ENV RUSTFLAGS="-A warnings"
RUN cargo build --target x86_64-unknown-linux-gnu --release

FROM nvidia/cuda:12.8.0-runtime-ubuntu24.04
RUN apt update && apt install -y openssh-server && \
    rm -rf /var/lib/apt/lists/* && \
    mkdir -p /var/run/sshd /workspace && \
    sed -i 's/#PermitRootLogin prohibit-password/PermitRootLogin yes/' /etc/ssh/sshd_config && \
    echo 'PasswordAuthentication no' >> /etc/ssh/sshd_config
COPY --from=builder /build/target/x86_64-unknown-linux-gnu/release/miner-launcher /usr/local/bin/
RUN chmod +x /usr/local/bin/miner-launcher

RUN echo '#!/bin/bash' > /entrypoint.sh && \
    echo '/usr/sbin/sshd' >> /entrypoint.sh && \
    echo 'exec /usr/local/bin/miner-launcher --account-token "${ACCOUNT_TOKEN}"' >> /entrypoint.sh && \
    chmod +x /entrypoint.sh

WORKDIR /workspace
CMD ["/entrypoint.sh"]