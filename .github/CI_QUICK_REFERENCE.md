# CI/CD Quick Reference

## Pre-Push Checklist

Before pushing to main or creating a PR:

```bash
# Run all checks that CI will run
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cd macos-app && swift build && cd ..
```

Or run this one-liner:
```bash
cargo test --workspace && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && (cd macos-app && swift build)
```

## Creating a Release

### 1. Ensure tests pass on main
```bash
git checkout main
git pull origin main
cargo test --workspace
```

### 2. Create a version tag
```bash
# Format: vMAJOR.MINOR.PATCH
git tag v0.2.0
git push origin v0.2.0
```

### 3. GitHub Actions automatically:
- Runs pre-release tests
- Builds release binaries and app bundle
- Generates SHA256 checksums
- Creates GitHub Release with artifacts and notes

### 4. Download from GitHub Releases tab

## Workflow Triggers

| Trigger | Workflow | Files |
|---------|----------|-------|
| Push to `main` | CI | `.github/workflows/ci.yml` |
| PR to `main` | CI | `.github/workflows/ci.yml` |
| Push tag `v*` | Release | `.github/workflows/release.yml` |

## Local Debugging

### If CI test fails
```bash
cd /path/to/repo
cargo test --workspace --verbose
RUST_TEST_THREADS=1 cargo test --workspace --verbose
```

### If linting fails
```bash
# Check formatting
cargo fmt --all -- --check

# View formatting changes
cargo fmt --all -- --diff

# Auto-fix formatting
cargo fmt --all

# Check clippy
cargo clippy --workspace --all-targets -- -D warnings

# Auto-fix clippy issues
cargo clippy --fix --allow-dirty
```

### If Swift build fails
```bash
cd macos-app
swift build -v
# Verbose output shows compilation details
```

## Artifact Locations in Release

After a release is created on GitHub:

1. **Navigate to**: https://github.com/username/intern/releases
2. **Find**: Latest release (marked as "Latest")
3. **Download**:
   - `intern-X.Y.Z-darwin-arm64.tar.gz` - Rust binaries
   - `Intern-X.Y.Z.zip` - macOS app bundle
   - `CHECKSUMS.txt` - SHA256 verification

## Common Issues

### "cargo test failed"
- Check if you're in the workspace root
- Ensure Cargo.lock is up to date
- Try `cargo clean && cargo test`

### "cargo clippy has warnings"
- Run `cargo clippy --fix --allow-dirty` to auto-fix
- Some warnings require manual fixes (clippy will guide you)

### "Swift build failed"
- Ensure you have the latest Xcode Command Line Tools
- Run `xcode-select --install` if needed
- Try `cd macos-app && rm -rf .build && swift build`

### "Release tag validation failed"
- Ensure tag matches `v` followed by semantic version
- Valid: `v1.0.0`, `v0.1.0`, `v2.3.4-alpha`
- Invalid: `v1`, `release-1.0`, `1.0.0` (missing 'v')

## Performance Tips

### Speed up cargo builds
- Use `cargo build --release -j$(nproc)` to parallelize
- Enable incremental compilation in Cargo.toml:
  ```toml
  [profile.dev]
  incremental = true
  ```

### Speed up CI locally
- Run checks in parallel in separate terminals:
  ```bash
  # Terminal 1
  cargo test --workspace

  # Terminal 2
  cargo clippy --workspace --all-targets -- -D warnings

  # Terminal 3
  cargo fmt --all -- --check

  # Terminal 4
  cd macos-app && swift build
  ```

### Cache optimization
- GitHub Actions automatically caches Cargo dependencies
- Local builds benefit from same caching
- Delete cache with `cargo clean` if needed

## Release Checklist

- [ ] All tests pass locally
- [ ] Code is formatted (`cargo fmt --all`)
- [ ] No clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`)
- [ ] Swift app builds (`cd macos-app && swift build`)
- [ ] Commits are meaningful and descriptive
- [ ] New tag follows semantic versioning (`vX.Y.Z`)
- [ ] Tag is pushed to GitHub (`git push origin vX.Y.Z`)
- [ ] Monitor GitHub Actions for release job completion
- [ ] Verify GitHub Release was created with all artifacts
- [ ] Spot check: Download and verify one artifact locally

## Monitoring Workflow Status

### GitHub Web UI
1. Go to repository
2. Click "Actions" tab
3. Workflows listed with status indicators
4. Click on specific workflow run to see details

### Command Line
```bash
# View recent commits and their workflow status
git log --oneline -20

# Check status of a specific workflow (requires gh CLI)
gh run list --workflow=ci.yml
gh run list --workflow=release.yml
```

## Need Help?

- GitHub Actions Docs: https://docs.github.com/actions
- Rust Testing: https://doc.rust-lang.org/book/ch11-00-testing.html
- Swift Building: https://www.swift.org/getting-started/
- Project Docs: See `CLAUDE.md` in repo root
