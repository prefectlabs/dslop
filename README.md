# dslop

Detect AI writing patterns (slop) in your codebase.

`dslop` is a fast CLI linter for prose. It flags telltale LLM writing patterns
(em-dashes, double-hyphen dashes, "it's not X, it's Y" constructions) and
statistical tells (uniform sentence rhythm, flat word-frequency distributions)
that human writing rarely produces.

## Install

```sh
pip install dslop
# or
uvx dslop .
```

## Use

```sh
dslop                       # check current directory
dslop README.md docs/       # check specific paths
dslop --config dslop.toml   # use a specific config

# Read from stdin:
echo "It's not a tool — it's a platform." | dslop -
pbpaste | dslop                                         # clipboard
git show HEAD:README.md | dslop                         # a past revision
```

Exits non-zero on violations, so it drops into CI or pre-commit unchanged.

### Pre-commit

```yaml
repos:
  - repo: https://github.com/prefectlabs/dslop
    rev: v0.1.0
    hooks:
      - id: dslop
```

## Configure

Create `dslop.toml` at your repo root:

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
