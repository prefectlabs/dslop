# sf

Detect AI writing patterns (slop) in your codebase.

`sf` is a fast CLI linter for prose. It flags telltale LLM writing patterns
(em-dashes, "it's not X, it's Y" constructions) and statistical tells
(uniform sentence rhythm, flat word-frequency distributions) that human
writing rarely produces.

## Install

```sh
pip install sf
# or
uvx sf .
```

## Use

```sh
sf                   # check current directory
sf README.md docs/   # check specific paths
sf --config sf.toml  # use a specific config
```

Exits non-zero on violations, so it drops into CI or pre-commit unchanged.

### Pre-commit

```yaml
repos:
  - repo: https://github.com/prefectlabs/sf
    rev: v0.1.0
    hooks:
      - id: sf
```

## Configure

Create `sf.toml` at your repo root:

```toml
[patterns]
em-dash = true
double-hyphen = true
contrastive = true

[metrics]
sentence-length-cv = 0.3
sentence-length-kurtosis = 1.5

[metrics.word-freq-dispersion]
threshold = 0.6
chunk-size = 200
top-n = 20
```

Each metric accepts a threshold (f64) or `false` to disable.

## License

Apache-2.0. See [LICENSE](./LICENSE).
