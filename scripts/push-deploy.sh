#!/bin/bash
# FlowLink push + deploy
# Usage: ./scripts/push-deploy.sh
set -e

cd "$(git rev-parse --show-toplevel)"
BRANCH=$(git rev-parse --abbrev-ref HEAD)

if [ "$BRANCH" != "main" ]; then
    echo "⚠️  Not on main branch (current: $BRANCH). Pushing without deploy."
    git push origin "$BRANCH"
    exit 0
fi

echo "📦 Pushing to GitHub..."
git push origin main

echo "🚀 Deploying to VPS..."
tar czf /tmp/fl-src.tar.gz \
    --exclude="target" --exclude=".git" --exclude="video" \
    --exclude="website" --exclude="docs" --exclude="scripts" \
    --exclude='*.md' \
    Cargo.toml Cargo.lock crates/

scp -q /tmp/fl-src.tar.gz root@93.93.207.44:/tmp/
rm -f /tmp/fl-src.tar.gz

ssh root@93.93.207.44 '
    cd /root/fl-build
    tar xzf /tmp/fl-src.tar.gz
    sed -i "s/features = \[\"vendored\"\]/features = []/" crates/relay/Cargo.toml
    . /root/.cargo/env
    bash /root/fl-build/scripts/deploy-vps.sh
'

echo "✅ Push + Deploy complete!"
