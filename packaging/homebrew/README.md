# Homebrew formula

`sqlv.rb` packages the `sqlv` CLI (and `sqlv-mcp` when released together)
as a Homebrew formula targeting macOS (arm64 + x86_64) and Linux (x86_64).

## One-time tap setup

```sh
# Create the tap repo on GitHub if it doesn't exist yet:
gh repo create shizhigu/homebrew-sqlv --public \
  --description "Homebrew tap for sqlv (SQLite viewer)"

# Clone + add the formula:
git clone https://github.com/shizhigu/homebrew-sqlv
cp sqlv.rb homebrew-sqlv/Formula/sqlv.rb
cd homebrew-sqlv && git add Formula/sqlv.rb && git commit -m "sqlv 0.1.0" && git push
```

## Install from the tap

```sh
brew tap shizhigu/sqlv
brew install sqlv
sqlv --version
```

## Refresh for a new release

1. Tag and push `vX.Y.Z` — the release workflow publishes per-target
   tarballs to the GitHub release page.
2. Compute SHA-256 of each tarball:
   ```sh
   for t in aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu; do
     curl -sL "https://github.com/shizhigu/sqlite-viewer/releases/download/vX.Y.Z/sqlv-vX.Y.Z-$t.tar.gz" \
       | shasum -a 256 | awk '{print "'"$t"': " $1}'
   done
   ```
3. Update `version` and the three `sha256` lines in `sqlv.rb`.
4. Commit + push to the tap — `brew update && brew upgrade sqlv` picks it up.

## Future: homebrew-core

Once the project has > 50 GitHub stars and a few months of stability,
we can open a PR to `Homebrew/homebrew-core` so users don't need the
tap at all.
