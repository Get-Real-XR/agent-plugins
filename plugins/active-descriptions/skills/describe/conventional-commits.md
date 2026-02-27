# Conventional Commits Format

The structural format for all change descriptions. This gives machines what
they need (parseable types, scopes, breaking change signals). The content
philosophy in [SKILL.md](SKILL.md) gives humans what they need.

```
<type>[optional scope][!]: <description>

[optional body]

[optional footer(s)]
```

## Types

- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation only
- `style` — formatting, no code change
- `refactor` — neither fixes nor adds functionality
- `perf` — performance improvement
- `test` — adding/fixing tests
- `build` — build system or dependencies
- `ci` — CI configuration
- `chore` — other changes that don't modify src/test

## Rules

1. Type prefix is REQUIRED. `feat` and `fix` trigger version bumps.
2. Scope is optional, parenthesized: `fix(parser): handle edge case`.
3. `!` after type/scope signals a breaking change: `feat(api)!: new auth flow`.
4. Description: imperative mood, lowercase, no period.
5. Body: one blank line after description. Free-form, explains *why*.
6. Footer: one blank line after body. `Token: value` or `Token #value`.
   - `BREAKING CHANGE: description` — alternative to `!` prefix.
   - `Reviewed-by: name`, `Refs: #123`, etc.
7. Breaking changes MUST be indicated via `!` in the type prefix OR a
   `BREAKING CHANGE:` footer (or both).

## Examples

```
feat(parser): add support for nested arrays

The previous parser only handled flat arrays. This adds recursive
descent parsing for arbitrarily nested structures.

Refs: #245
```

```
fix!: correct off-by-one in buffer allocation

The allocation size was one byte short for inputs whose length is
an exact multiple of the chunk size. This caused UB when the last
chunk was written.

BREAKING CHANGE: Buffer::new() now takes usize instead of u32.
```
