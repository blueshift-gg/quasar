#!/usr/bin/env bash
# Run this after making the repo public.
# Requires: gh auth login with admin scope.
set -euo pipefail

REPO="blueshift-gg/quasar"
BRANCH="master"

echo "Setting up branch protection for $REPO:$BRANCH..."

gh api "repos/$REPO/branches/$BRANCH/protection" \
  --method PUT \
  --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Rustfmt",
      "Clippy",
      "Check Features",
      "Build & Test"
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "dismiss_stale_reviews": true
  },
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_linear_history": false,
  "required_conversation_resolution": false
}
EOF

echo "Done. Branch protection applied to $BRANCH."
echo ""
echo "Rules:"
echo "  - PRs required with 1 approval"
echo "  - Stale reviews dismissed on new push"
echo "  - CI must pass (fmt, clippy, features, build+test)"
echo "  - No force pushes"
echo "  - No branch deletion"
