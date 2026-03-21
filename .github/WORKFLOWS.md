# GitHub Actions Workflows

This document describes the CI/CD pipelines for Intern.

## Workflows Overview

### 1. CI Pipeline (`.github/workflows/ci.yml`)

Runs on every push to `main` and pull requests targeting `main`.

#### Jobs (Parallel Execution)

**Test Suite** (`test`)
- Runs `cargo test --workspace --verbose`
- Sets `RUST_TEST_THREADS=1` for serial execution
- Sets `CI=true` environment variable
- Caches Cargo registry, index, and build artifacts
- Target: `aarch64-apple-darwin` (Apple Silicon)

**Linting** (`lint`)
- Runs `cargo fmt --all -- --check` (formatting validation)
- Runs `cargo clippy --workspace --all-targets -- -D warnings` (strict linting)
- Installs rustfmt and clippy components
- Caches Cargo registry, index, and build artifacts
- Fails on any clippy warnings (treated as errors)

**Swift Build** (`swift-build`)
- Runs `cd macos-app && swift build --verbose`
- Validates Swift compilation without binaries
- Caches Swift build artifacts in `.build/`
- Ensures app builds successfully

**CI Pass** (`ci-pass`)
- Final status check that depends on all jobs
- Only runs if test, lint, and swift-build all pass
- Provides a single status indicator for the entire CI pipeline

#### Runner

- `macos-14` (GitHub-hosted macOS 14, Apple Silicon)

#### Caching Strategy

- **Cargo Registry Cache**: `~/.cargo/registry` keyed on `Cargo.lock`
- **Cargo Index Cache**: `~/.cargo/git` keyed on `Cargo.lock`
- **Cargo Build Cache**: `target/` keyed on `Cargo.lock`
- **Swift Build Cache**: `macos-app/.build/` keyed on `Package.swift`

Each cache has restore-keys for graceful degradation if exact match not found.

---

### 2. Release Pipeline (`.github/workflows/release.yml`)

Triggers on version tags matching pattern `v*` (e.g., `v0.1.0`, `v1.2.3`).

#### Jobs (Sequential Execution)

**Pre-Release Tests** (`test`)
- Runs full test suite with same config as CI
- Ensures all tests pass before building release
- Prevents broken releases
- Caches Cargo artifacts for faster builds

**Build Release** (`build`)
- Depends on `test` job passing
- Extracts version from git tag (validates semver format)
- Builds release binaries:
  - `cargo build --release`
  - Produces: `intern`, `intern-runner`, `intern`
- Builds Swift app:
  - `swift build -c release`
  - Runs `build-app.sh` to create `.app` bundle
- Packages artifacts:
  - **Tarball**: `intern-{VERSION}-darwin-arm64.tar.gz`
    - Contains 3 Rust binaries
    - Optimized for binary installation
  - **App Bundle**: `Intern-{VERSION}.zip`
    - Contains `Intern.app`
    - Ready to drag-and-drop into Applications folder
- Generates SHA256 checksums for both artifacts
- Outputs version for downstream jobs
- Uploads artifacts to GitHub Actions (7-day retention)

**Create GitHub Release** (`release`)
- Depends on `build` job completing
- Downloads artifacts from `build` job
- Generates release notes:
  - Lists both artifacts with descriptions
  - Provides installation instructions for both distribution formats
  - Includes commit log since previous release
  - Auto-detects previous release from git tags
- Creates GitHub Release with:
  - Tag: `v{VERSION}`
  - Name: `Release v{VERSION}`
  - Body: Auto-generated release notes
  - Artifacts: Tarball, app zip, checksums
  - Marked as latest release
  - Not a draft or prerelease

**Post-Release Verification** (`post-release`)
- Depends on both `build` and `release` jobs
- Final validation step
- Confirms all artifacts published successfully

#### Runner

- `macos-14` (GitHub-hosted macOS 14, Apple Silicon)

#### Permissions

- `contents: write` required for creating GitHub Releases

#### Caching Strategy

Same as CI pipeline, with separate cache for release builds (`cargo-build-target-release-*`).

---

## Version Format

Tags must follow semantic versioning:
- Valid: `v0.1.0`, `v1.0.0`, `v2.3.4`, `v1.0.0-alpha`, `v1.0.0-beta.1`
- Invalid: `v0.1`, `vrelease`, `1.0.0` (missing 'v' prefix)

The release pipeline validates this and fails if tag format is invalid.

---

## Environment Variables

### Global (Both Workflows)
- `CARGO_TERM_COLOR: always` - Colorized Cargo output
- `RUST_BACKTRACE: 1` - Rust panic backtrace enabled

### CI Workflow
- `RUST_TEST_THREADS: 1` - Serial test execution
- `CI: true` - Signals to tests that they run in CI (skips integration tests)

---

## Artifact Artifacts

### CI Workflow
- Build artifacts cached in GitHub Actions
- No artifacts published externally

### Release Workflow

**GitHub Actions Artifacts** (temporary, 7-day retention)
- `release-artifacts/`
  - `intern-{VERSION}-darwin-arm64.tar.gz`
  - `Intern-{VERSION}.zip`
  - `CHECKSUMS.txt`

**GitHub Releases** (permanent)
- All artifacts uploaded to GitHub Release page
- Downloadable via GitHub UI or API
- Associated with release tag

---

## Installation Methods

### From Release Artifacts

**Option 1: Binary Installation (Tarball)**
```bash
# Download and extract
tar -xzf intern-{VERSION}-darwin-arm64.tar.gz

# Install to PATH
sudo mv intern intern-runner intern /usr/local/bin/

# Verify
intern --version
```

**Option 2: App Bundle Installation (Zip)**
```bash
# Download and extract
unzip Intern-{VERSION}.zip

# Install to Applications folder
mv "Intern.app" /Applications/

# Or run directly
open "Intern.app"
```

**Option 3: Verify Checksums**
```bash
# Verify integrity of downloaded artifacts
sha256sum -c CHECKSUMS.txt

# Expected output:
# intern-{VERSION}-darwin-arm64.tar.gz: OK
# Intern-{VERSION}.zip: OK
```

---

## Failure Modes and Recovery

### CI Pipeline Failures

**Test Failures**
- Workflow stops at test stage
- PR cannot be merged until tests pass
- Fix: Resolve test failures in branch

**Lint/Clippy Failures**
- Workflow stops at lint stage
- Fix: Run locally:
  ```bash
  cargo fmt --all        # Auto-fix formatting
  cargo clippy --fix     # Auto-fix clippy issues
  git add -A && git commit -m "chore: format and lint fixes"
  ```

**Swift Build Failures**
- Workflow stops at swift-build stage
- Fix: Debug locally in `macos-app/`:
  ```bash
  cd macos-app && swift build -v
  ```

### Release Pipeline Failures

**Pre-Release Tests Fail**
- Release does not proceed
- Fix: Ensure all tests pass on the tagged commit:
  ```bash
  git checkout v{VERSION}
  cargo test --workspace
  ```

**Build Fails**
- No artifacts created, no release published
- Fix: Debug on `macos-14` equivalent, push new commit, create new tag

**Release Creation Fails**
- Artifacts built successfully but GitHub Release not created
- Fix: Manually create GitHub Release and upload artifacts, or re-run job

---

## Monitoring and Debugging

### View Workflow Runs
1. Go to repository on GitHub
2. Click "Actions" tab
3. Select workflow from list
4. Click specific run to view details

### View Logs
- Click on any job to expand detailed logs
- Scroll through step output to find errors

### Local Testing

**Test Locally Before PR**
```bash
# Run all CI checks locally
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cd macos-app && swift build
```

**Test Release Build Locally**
```bash
cargo build --release
cd macos-app && swift build -c release && bash build-app.sh
```

---

## Future Enhancements

### Code Signing (Placeholder)
When adding code signing:
1. Provision Apple Developer Certificate
2. Create GitHub secret: `APPLE_DEVELOPER_ID_APPLICATION`
3. Add signing step in release workflow:
   ```yaml
   - name: Sign binaries
     run: |
       codesign -s "${{ secrets.APPLE_DEVELOPER_ID_APPLICATION }}" \
         dist/intern dist/intern-runner dist/intern
   ```
4. Update release notes with signature verification instructions

### Notarization
When macOS notarization needed:
1. Add Apple ID credentials to GitHub Secrets
2. Notarize .app bundle before release
3. Staple notarization ticket to app

### Additional Platforms
To add Linux/Windows support:
1. Create additional workflow for other platforms
2. Update release workflow to accept multiple OS artifacts
3. Modify release notes to list artifacts per platform

### Automated Changelog
Consider integrating:
- `git-cliff` for semantic changelog generation
- `changelog-rs` for automated entries
- GitHub Release notes auto-generation (already partially implemented)

---

## Best Practices

1. **Always run CI checks locally before pushing**
   ```bash
   cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
   ```

2. **Use semantic versioning for tags**
   - Patch: `v1.0.1` (bug fixes)
   - Minor: `v1.1.0` (new features, backward compatible)
   - Major: `v2.0.0` (breaking changes)

3. **Include meaningful commit messages**
   - Release workflow includes commit log in release notes
   - Use descriptive messages for better documentation

4. **Verify checksums after download**
   - Always verify artifact integrity
   - Checksums provided in releases

5. **Keep dependencies updated**
   - Periodically run `cargo update`
   - Test thoroughly before committing Cargo.lock changes

---

## Support

For issues with workflows:
1. Check GitHub Actions logs for specific error messages
2. Reproduce locally to debug
3. Review workflow YAML syntax
4. Check GitHub Actions documentation: https://docs.github.com/actions
