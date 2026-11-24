# The RegelHet Fundament plugin. Build from this directory:
#   docker build -t <registry>/regelhet-fundament-plugin:<tag> packages/fundament-plugin
FROM golang:1.27.3-alpine@sha256:fa3d596898d9f126af7c01d14ee7597bab3f39cccf392ef36b69fe77cd681f18 AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY *.go ./
COPY api ./api
COPY crds ./crds
COPY console ./console
# snapshot.go embeds it.
COPY snapshot.sh ./
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /regelhet-plugin .

FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
RUN addgroup -S plugin && adduser -S plugin -G plugin
COPY --from=build /regelhet-plugin /usr/local/bin/regelhet-plugin
COPY definition.yaml /app/definition.yaml
WORKDIR /app
USER plugin
ENTRYPOINT ["regelhet-plugin"]
