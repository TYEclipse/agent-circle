# Agent Circle — task runner
# https://github.com/casey/just

_default:
    @just --list

# Build debug binary
build:
    cargo build

# Build release binary
build-release:
    cargo build --release

# Run all tests (unit + integration)
test:
    cargo test --all-targets

# Run only ignored integration tests
test-integration:
    cargo test --all-targets -- --ignored

# Run tests with output
test-verbose:
    cargo test --all-targets -- --nocapture

# Format code
fmt:
    cargo fmt --all

# Check formatting (CI mode)
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Fix auto-fixable clippy warnings
fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty

# Full CI pipeline locally
ci: fmt-check lint test
    @echo "✅ CI pipeline passed"

# License audit
deny:
    cargo deny check bans licenses sources

# Security audit
audit:
    cargo audit

# Run daemon with debug logging
daemon group="":
    RUST_LOG=debug cargo run -- daemon start {{if group == "" { "" } else { "--group " + group }}}

# Create identity
identity-create name owner model="deepseek-v4" capabilities="":
    cargo run -- identity create --name "{{name}}" --owner "{{owner}}" --model "{{model}}" {{capabilities}}

# Show current identity
identity-show:
    cargo run -- identity show

# Clean build artifacts
clean:
    cargo clean

# Watch for changes and run tests
watch:
    cargo watch -x test

# Generate code coverage (requires cargo-tarpaulin)
coverage:
    cargo tarpaulin --out Html --output-dir target/coverage

# Check for outdated dependencies
outdated:
    cargo outdated

# Update dependencies
update:
    cargo update

# Count lines of code
loc:
    tokei src/ tests/ --sort=lines

# Show dependency tree
tree:
    cargo tree

# Release (prompts for version)
release version:
    ./scripts/release.sh {{version}}
