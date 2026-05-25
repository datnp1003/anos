# 🦾 Anos — Static Musl Build + Runtime
# Multi-stage: build static binary, then minimal runtime

FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev

WORKDIR /build
COPY anosd/ anosd/
COPY anos-cli/ anos-cli/

RUN cd anosd && cargo build --release --target $(rustc -vV | grep host | awk '{print $2}') && \
    cd ../anos-cli && cargo build --release --target $(rustc -vV | grep host | awk '{print $2}')

# ── Runtime ──
FROM alpine:3.21

LABEL org.opencontainers.image.source="https://github.com/datnp1003/anos"
LABEL org.opencontainers.image.description="Anos — AI Native OS"

RUN adduser -D anos

COPY --from=builder /build/anosd/target/*/release/anosd /usr/local/bin/anosd
COPY --from=builder /build/anos-cli/target/*/release/anos-cli /usr/local/bin/anos-cli

COPY ANOS-SYSTEM-PROMPT.md /opt/anos/
COPY skills/ /opt/anos/skills/
COPY config/ /opt/anos/config/

RUN chmod +x /usr/local/bin/anosd /usr/local/bin/anos-cli && \
    chown -R anos:anos /opt/anos

ENV ANOS_DIR=/opt/anos
EXPOSE 8787

COPY docker/docker-entrypoint.sh /usr/local/bin/
USER anos
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
