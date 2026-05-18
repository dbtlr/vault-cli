## Summary

A short description of the change and why. One or two sentences is usually enough; the commit history carries the detail.

## Type of change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds capability)
- [ ] Breaking change (fix or feature that changes existing behavior, output, or schema)
- [ ] Documentation only
- [ ] Internal refactor (no behavior change)

## Testing

How you verified this works. Include the commands you ran.

```bash
just verify
```

## Checklist

- [ ] `just verify` passes locally.
- [ ] CHANGELOG.md updated if this change is user-visible (added / changed / removed / fixed).
- [ ] Docs updated if this change affects commands, configuration, or output schemas.
- [ ] No personal-vault names or private examples leaked into committed files.
- [ ] No new security-sensitive surface (filesystem writes, network access, etc.) introduced without discussion.

## Related issues

Closes #
