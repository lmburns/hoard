FROM rust:alpine AS build

RUN apk add build-base
COPY . .
RUN cargo build

FROM python:alpine

ENV CI=true GITHUB_ACTIONS=true
COPY ci-tests ci-tests
COPY --from=build target/debug/hoard target/debug/hoard

RUN python3 ci-tests/test.py last_paths