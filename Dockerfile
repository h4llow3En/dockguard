FROM gcr.io/distroless/cc-debian12:nonroot

ARG BINARY_PATH=target/debug/dockguard
COPY ${BINARY_PATH} /usr/local/bin/dockguard

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD ["/usr/local/bin/dockguard", "--healthcheck"]

ENTRYPOINT ["/usr/local/bin/dockguard"]
