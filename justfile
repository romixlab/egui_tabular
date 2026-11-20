# Bare `just` only lists the recipes
default:
    @just --list

# The tests. `just test`: all of them (test-all), the run done before merging to main. `just test --changed`: only the
# workspace crates changed against main and the crates that depend on them, what to run
# before sending work up for review.
test *flags:
    if [ "{{flags}}" = "--changed" ]; then if command -v changedtests >/dev/null 2>&1; then f=$(changedtests) && if [ -n "$f" ]; then cargo nextest run --workspace -E "$f"; fi; else echo "changedtests not found; running the full test suite instead" >&2; {{just_executable()}} test-all; fi; else {{just_executable()}} test-all {{flags}}; fi

test-all:
    cargo test --workspace

lint:
    cargo fmt --check
    cargo clippy --workspace --all-targets

dry-publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude row_spans --exclude simple --exclude tests --dry-run --allow-dirty

publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude row_spans --exclude simple --exclude tests
