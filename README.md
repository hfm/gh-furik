# gh-furik

It's a Rust implementation of the GitHub CLI extension for digging into your GitHub activity over a period.
It's inspired by the original projects [pepabo/furik](https://github.com/pepabo/furik) and [kenchan/gh-furik](https://github.com/kenchan/gh-furik).

## Installation

```bash
gh extension install hfm/gh-furik
```

## Usage

```bash
gh furik --from 2025-01-01 --to 2025-01-31
gh furik --hostname ghe.example.com --from 2025-02-01 --to 2025-02-28
gh furik --compact --from 2025-03-01 --to 2025-03-07
```

Options:
- `--from YYYY-MM-DD` start date
- `--to YYYY-MM-DD` end date
- `--hostname HOST` target hostname (default: github.com)
- `-c, --compact` compact list output
- Authentication is resolved per host:
  - For `github.com`: `GH_TOKEN` / `GITHUB_TOKEN`
  - For other hosts (GHES): `GH_ENTERPRISE_TOKEN` / `GITHUB_ENTERPRISE_TOKEN`
  - If not set, it falls back to `gh auth token --secure-storage --hostname <HOST>`

## Development

- Format GraphQL queries: `dprint fmt` (requires [dprint](https://dprint.dev/))

## License

[MIT License](LICENSE).
