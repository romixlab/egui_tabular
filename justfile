# Bare `just` only lists the recipes
default:
    @just --list

test:
    cargo test --workspace

lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets

dry-publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude simple --exclude tests --dry-run --allow-dirty

publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude simple --exclude tests
