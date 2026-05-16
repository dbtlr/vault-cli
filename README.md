# vault-cli

Experimental Rust CLI for deterministic Markdown vault graph indexing.

The current binary name is `vault`.

## v0 Scope

```bash
vault graph documents --root <path> --format jsonl
vault graph documents --root <path> --filter status:draft --format jsonl
vault graph build --root <path> --cache .vault/cache --format json
vault graph links --root <path> --format jsonl
vault graph unresolved --root <path> --format jsonl
vault graph backlinks <path-or-stem> --root <path> --format jsonl
vault graph inspect <path-or-stem> --root <path> --format json
```

The first pass is stateless and read-only. It walks Markdown files, parses generic frontmatter, extracts headings, extracts Markdown links and wikilinks, and resolves links against vault-relative paths or unique note stems.
