#!/usr/bin/env bash
# Release script for agent-circle
# Usage: ./scripts/release.sh <new-version>
# Example: ./scripts/release.sh 0.3.0

set -euo pipefail

NEW_VERSION="${1:-}"
if [ -z "$NEW_VERSION" ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 0.3.0"
    exit 1
fi

# Strip leading 'v' if present
NEW_VERSION="${NEW_VERSION#v}"

# Ensure we're on master
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "master" ]; then
    echo "❌  Must be on master branch (current: $BRANCH)"
    exit 1
fi

# Ensure clean working directory
if ! git diff-index --quiet HEAD --; then
    echo "❌  Working directory is not clean. Commit or stash changes first."
    exit 1
fi

# Pull latest
echo "📥 Pulling latest master..."
git pull origin master

# Verify current version
CURRENT_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "📦 Current version: v$CURRENT_VERSION → v$NEW_VERSION"

# Confirm
read -rp "🚀 Proceed with release? [y/N] " CONFIRM
if [ "$CONFIRM" != "y" ] && [ "$CONFIRM" != "Y" ]; then
    echo "❌  Aborted."
    exit 1
fi

# Bump version in Cargo.toml
sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Verify format + lint
echo "🔍 Running fmt + clippy..."
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
echo "🧪 Running tests..."
cargo test --all-targets

# Build release
echo "🔨 Building release..."
cargo build --release

# Run deny + audit
echo "🔒 License audit..."
cargo deny check bans licenses sources 2>&1 | tail -1
echo "🛡️ Security audit..."
cargo audit 2>&1 | tail -3 || true

# Commit
git add Cargo.toml
git commit -m "🔖 Release v$NEW_VERSION"
git tag "v$NEW_VERSION"

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║  🚀 Release v$NEW_VERSION ready!         ║"
echo "╠══════════════════════════════════════════╣"
echo "║  git push origin master                  ║"
echo "║  git push origin v$NEW_VERSION           ║"
echo "╚══════════════════════════════════════════╝"
echo ""
read -rp "📤 Push to GitHub? [y/N] " PUSH
if [ "$PUSH" = "y" ] || [ "$PUSH" = "Y" ]; then
    git push origin master
    git push origin "v$NEW_VERSION"
    echo "✅ v$NEW_VERSION released!"
else
    echo "⚠️  Tag created locally. Push manually when ready."
fi
