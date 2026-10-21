## Summary

<!-- What does this change and why? Link related issues (e.g. "Fixes #12"). -->

## Testing

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] Tested manually on a game (name, store, GPU):

## Checklist

- [ ] Any file placed in a game folder goes through `installer::Deployment`, so uninstall can roll it back
- [ ] New behavior has a test (installer and download changes especially)
- [ ] README / CHANGELOG updated if user-visible behavior changed
