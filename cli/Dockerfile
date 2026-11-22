# The command-line tool as a small image for CI (design: CLI). The release workflow builds it from the Linux archives
# it made (amd64 and arm64) and pushes ghcr.io/<owner>/tlmbench:<version>; nothing is compiled here.
#   docker run --rm -v "$PWD:/data" ghcr.io/<owner>/tlmbench check /data/run.tlm
FROM debian:bookworm-slim
ARG TARGETARCH
COPY dist/${TARGETARCH}/tlmbench /usr/local/bin/tlmbench
WORKDIR /data
ENTRYPOINT ["tlmbench"]
