## Summary
<!-- Brief description of what changed and why -->

## Type of Change
<!-- Mark the relevant option with an "x" -->

- [ ] Bug fix (fixes an issue without changing existing functionality)
- [ ] Feature (adds new functionality)
- [ ] Breaking change (modifies behavior or API in an incompatible way)
- [ ] Documentation (documentation or README update)
- [ ] Refactoring (code quality improvement without behavioral change)
- [ ] Test (adds or modifies tests)

## Testing Checklist

- [ ] Ran `cargo test --workspace` and all tests pass
- [ ] Built the macOS app: `cd macos-app && swift build`
- [ ] Tested the CLI with manual commands
- [ ] Verified daemon health and socket communication
- [ ] Tested with fresh `~/.intern/` directory if schema changes
- [ ] Checked logs in SQLite if database changes involved

## Related Issues

Closes #(issue number if applicable)

## Additional Notes
<!-- Any additional context or edge cases to consider -->
