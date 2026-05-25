# 🦾 Anos — AI Native OS Container
# Phase 1: Container image for dev/test/deploy

FROM debian:stable-slim

LABEL org.opencontainers.image.source="https://github.com/datnp1003/anos"
LABEL org.opencontainers.image.description="Anos — AI Native OS"

# Minimal runtime deps
RUN apt-get update -qq && \
    apt-get install -y -qq --no-install-recommends \
        ca-certificates \
        curl \
        git \
    && rm -rf /var/lib/apt/lists/*

# Create anos user
RUN useradd -m -s /bin/bash anos

# Copy binaries (static musl — universal Linux)
COPY anosd /usr/local/bin/anosd
COPY anos-cli /usr/local/bin/anos-cli

# Copy runtime assets
COPY install.sh /opt/anos/
COPY ANOS-SYSTEM-PROMPT.md /opt/anos/
COPY skills/ /opt/anos/skills/
COPY config/ /opt/anos/config/

# Set permissions
RUN chmod +x /usr/local/bin/anosd /usr/local/bin/anos-cli && \
    chown -R anos:anos /opt/anos

ENV ANOS_DIR=/opt/anos
ENV ANOS_SOCKET=/tmp/anos.sock

# Expose SSE events port
EXPOSE 8787

# Health check via socket ping
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD echo "/ping" | /usr/local/bin/anos-cli --raw 2>/dev/null | grep -q pong || exit 1

USER anos

# Entrypoint: start daemon + keep alive
COPY docker-entrypoint.sh /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
