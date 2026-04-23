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
demonstrative-is = true
filler-adverbs = true          # quietly, actually, really, simply, essentially, ...
weasel-connectives = true      # which means, in turn, the reality is, ...
banned-flourish = true         # worth noting, to be clear, at the end of the day, ...
negation-pair = true           # "not X, not Y"
symmetric-negation = true      # "fine if X, wrong if Y" / "not X but Y"
which-chain = true             # 3+ "which" in a single sentence
but-opener = true              # short sentences starting with "But"
aphorism = true                # "X does not Y." one-liners
summary-capstone = true        # paragraph-final "That is the X."
declarative-callback = true    # paragraph-final fragment callbacks
three-beat = true              # 3 consecutive short sentences in a paragraph

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
