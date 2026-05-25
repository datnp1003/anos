# 🦾 Anos — Multi-stage: build IN Alpine, run IN Alpine
# No more cross-libc issues

FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev

WORKDIR /build
COPY anosd/ anosd/
COPY anos-cli/ anos-cli/

RUN cd anosd && cargo build --release && \
    cd ../anos-cli && cargo build --release

# ── Runtime ──
FROM alpine:3.21

RUN adduser -D anos
RUN apk add --no-cache ca-certificates

COPY --from=builder /build/anosd/target/release/anosd /usr/local/bin/anosd
COPY --from=builder /build/anos-cli/target/release/anos-cli /usr/local/bin/anos-cli

COPY ANOS-SYSTEM-PROMPT.md /opt/anos/
COPY skills/ /opt/anos/skills/
COPY config/ /opt/anos/config/

RUN chmod +x /usr/local/bin/anosd /usr/local/bin/anos-cli && \
    chown -R anos:anos /opt/anos

COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENV ANOS_DIR=/opt/anos
EXPOSE 8787

USER anos
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
