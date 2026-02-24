.PHONY: all build release test clean dist linux mac fmt check help

BINS     = knotcoind knotcoin-cli
ARCH    := $(shell uname -m)
OS      := $(shell uname -s)

all: build

## Build debug binaries (fast, includes debug symbols)
build:
	cargo build --bins

## Build release binaries (optimized, stripped)
release:
	cargo build --release --bins
	strip target/release/knotcoind target/release/knotcoin-cli

## Run all tests
test:
	cargo test

## Run tests with output
test-verbose:
	cargo test -- --nocapture

## Build and package for current platform
dist: release
ifeq ($(OS),Linux)
	bash contrib/build_linux.sh
else ifeq ($(OS),Darwin)
	bash contrib/build_mac.sh
else
	$(error Unsupported OS: $(OS))
endif

## Linux x86_64 release (run on Linux)
linux:
	bash contrib/build_linux.sh

## macOS arm64 release (run on an M-series Mac)
mac:
	bash contrib/build_mac.sh

## Start the node (debug build)
run:
	RUST_LOG=info cargo run --bin knotcoind

## Start with release binary
start: release
	./target/release/knotcoind

## Check formatting + clippy
check:
	cargo fmt --check
	cargo clippy --bins -- -D warnings

## Auto-fix formatting
fmt:
	cargo fmt

## Remove build artifacts
clean:
	cargo clean
	rm -rf dist/

## Show available targets
help:
	@grep -E '^## ' Makefile | sed 's/## /  /'
