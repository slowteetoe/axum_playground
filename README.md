# Axum playground
![build](https://github.com/slowteetoe/axum_playground/actions/workflows/rust.yml/badge.svg)


[Axum](https://github.com/tokio-rs/axum) is an ergonomic and modular web framework in Rust, built with Tokio, [Tower](https://crates.io/crates/tower) and [Hyper](https://crates.io/crates/hyper)

This is just a relatively simple hello world app to try things out, specifically:

- main server running on port 3000
  - GET / -> "hello world"
- metrics server running on port 3001 (prometheus format)
  - GET /metrics
- graceful shutdown of both servers in response to ctrl-c (it works, but take a little while, just be patient)
- distributed tracing using OpenTelmetry
  - configured (through ENV) to send spans to a local Jaeger instance via gRPC (can run `./local-tracing.sh` to start up an all-in-one jaeger instance in docker)

## Running

```sh
OTEL_SERVICE_NAME=axum-hello OTEL_TRACES_SAMPLER="always_on" OTEL_EXPORTER_OTLP_PROTOCOL="grpc" OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317" cargo run
```

## TODO

This is really rough, cleanup... especially OTEL startup/env code
Figure out dependencies - there were a lot added trying to figure out the error:

> OpenTelemetry trace error occurred. Exporter otlp encountered the following error(s): no http client, you must select one from features or provide your own implementation
