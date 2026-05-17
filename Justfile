set shell := ["bash", "-cu"]

default:
    @just --list

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

check:
    cargo check

test:
    cargo test

build:
    cargo build -p vault-cli

release:
    cargo build -p vault-cli --release

install:
    cargo install --path crates/vault-cli

verify: fmt-check test

run *args:
    cargo run -q -p vault-cli -- {{args}}

fixture-documents root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph documents --format jsonl

fixture-links root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph links --format jsonl

fixture-unresolved root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph unresolved --format json

fixture-diagnostics root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph diagnostics --format jsonl

fixture-backlinks target="beta" root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph backlinks '{{target}}' --format jsonl

fixture-inspect target="alpha.md" root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph inspect '{{target}}' --format json

fixture-build-cache cache="/tmp/vault-cli-cache" root="fixtures/basic":
    cargo run -q -p vault-cli -- -C '{{root}}' graph build --cache '{{cache}}' --format json
