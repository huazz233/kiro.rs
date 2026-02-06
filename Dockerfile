FROM node:24-alpine AS frontend-builder

WORKDIR /app/admin-ui
COPY admin-ui/package.json ./
RUN npm install -g pnpm && pnpm install
COPY admin-ui ./
RUN pnpm build

FROM rust:1.93-alpine AS builder

# 可选：启用敏感日志输出（仅用于排障）
ARG ENABLE_SENSITIVE_LOGS=false

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY --from=frontend-builder /app/admin-ui/dist /app/admin-ui/dist

RUN if [ "$ENABLE_SENSITIVE_LOGS" = "true" ]; then \
        cargo build --release --features sensitive-logs; \
    else \
        cargo build --release; \
    fi

FROM alpine:3.21

LABEL org.opencontainers.image.source=https://github.com/huazz233/kiro.rs
LABEL org.opencontainers.image.description="Anthropic Claude API compatible proxy service"
LABEL org.opencontainers.image.licenses=MIT

RUN apk add --no-cache ca-certificates

WORKDIR /app
COPY --from=builder /app/target/release/kiro-rs /app/kiro-rs

VOLUME ["/app/config"]

EXPOSE 8990

CMD ["./kiro-rs", "-c", "/app/config/config.json", "--credentials", "/app/config/credentials.json"]
