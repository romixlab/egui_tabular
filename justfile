# Bare `just` only lists the recipes
default:
    @just --list

# The tests. `just test`: all of them (test-all), the run of the `tpm land` gate. `just test --changed`: only the
# workspace crates changed against main and the crates that depend on them (tpm test changed), what a session runs
# before tpm work ready.
test *flags:
    if [ "{{flags}}" = "--changed" ]; then tpm test changed -- cargo nextest run --workspace; else {{just_executable()}} test-all {{flags}}; fi

test-all:
    cargo test --workspace

lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets

dry-publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude simple --exclude tests --dry-run --allow-dirty

publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude simple --exclude tests
