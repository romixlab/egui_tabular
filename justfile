dry-publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude simple --exclude tests --dry-run --allow-dirty

publish:
    cargo publish --workspace --exclude csv_xls_import --exclude derive_row --exclude simple --exclude tests
