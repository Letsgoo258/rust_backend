#!/bin/bash
# Custom script to safely format, test, commit, and push for full stack

set -e

echo "🚀 Starting full-stack commit and push process..."

# Step 1: Backend checks
echo "📦 Formatting and checking Backend (Rust)..."
cd backend
cargo fmt
cargo check
cargo test
cd ..

# Step 2: Frontend checks
echo "🎨 Formatting and checking Frontend (Next.js)..."
cd frontend
npm run lint
# npm test # Uncomment when frontend tests are added
cd ..

# Step 3: Add all changes
echo "➕ Adding changes to git..."
git add .

# Step 4: Check if there's anything to commit
if git diff-index --quiet HEAD --; then
    echo "⚠️ No changes to commit."
    exit 0
fi

# Step 5: Prompt for commit message
read -p "📝 Enter commit message: " commit_message

if [ -z "$commit_message" ]; then
    echo "❌ Error: Commit message cannot be empty."
    exit 1
fi

# Step 6: Commit
echo "💾 Committing changes..."
git commit -m "$commit_message"

# Step 7: Push
echo "☁️ Pushing to remote repository..."
current_branch=$(git rev-parse --abbrev-ref HEAD)
git push origin "$current_branch"

echo "✅ Successfully pushed full stack to $current_branch!"
