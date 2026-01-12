#!/usr/bin/env bash
set -euo pipefail

# Run development server with correct Node.js version
# This script automatically uses nvm if available and .nvmrc exists

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load nvm if available
if [ -s "$HOME/.nvm/nvm.sh" ]; then
  export NVM_DIR="$HOME/.nvm"
  # shellcheck source=/dev/null
  . "$NVM_DIR/nvm.sh"

  # Use Node version from .nvmrc if it exists
  if [ -f "$SCRIPT_DIR/.nvmrc" ]; then
    echo "📦 Using Node.js version from .nvmrc..."
    nvm use || {
      echo "⚠️  Node.js version not installed. Installing..."
      nvm install
    }
  fi
fi

# Run npm commands
echo ""
echo "📦 Installing dependencies..."
npm install

echo ""
echo "🚀 Starting dev server..."
npm run dev
