.PHONY: default build clean clippy db doc format image release run skeptic start stop test

CARGO_FLAGS := --features "$(FEATURES)"

default: build

build:
	cargo build $(CARGO_FLAGS)

clean:
	cargo clean

clippy:
	if $$CLIPPY; then cargo clippy $(CARGO_FLAGS); fi

doc: build
	cargo doc --no-deps $(CARGO_FLAGS)

format:
	cargo fmt

release:
	cargo build --release $(CARGO_FLAGS)

cpp:
	cargo clean -p ns_luis
	BUILD_CPP=1 cargo build $(CARGO_FLAGS)

run:
	cargo run

skeptic:
	USE_SKEPTIC=1 cargo test $(CARGO_FLAGS)

test: build
	cargo test $(CARGO_FLAGS)
