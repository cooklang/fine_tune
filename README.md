# Cooklang Fine-Tuning Dataset

Training data for fine-tuning LLM models to convert recipes into [Cooklang](https://cooklang.org) format.

The dataset is built from [HelloFresh](https://www.hellofresh.com) recipe pages scraped across multiple countries.

## Structure

```
recipes/           Converted recipe pairs (.recipe → .cook) used as training data
inbox/             Raw scraped recipes not yet converted
src/main.rs        HelloFresh scraper (Rust)
batch_convert.py   Batch conversion script using OpenAI Batch API
```

## Data

### `recipes/` — Training Pairs

~17,700 recipe pairs across 11 countries (at, au, be, de, es, fr, gb, ie, it, nl, us). Each recipe has:

- `.recipe` — raw structured text scraped from HelloFresh (YAML frontmatter + ingredients + steps)
- `.cook` — the same recipe converted to Cooklang format

These pairs form the training data for fine-tuning.

### `inbox/` — Unconverted Recipes

~19,700 raw `.recipe` files from 4 additional countries (ca, ch, lu, nz) awaiting conversion.

## Tools

### Scraper (Rust)

Scrapes HelloFresh sitemaps and downloads recipes using [cooklang-import](https://github.com/nicholasgasior/cooklang-import).

```sh
cargo run -- --countries us,gb --output recipes
cargo run -- --list-countries
```

Requires `cooklang-import` binary (default path: `../cooklang-import/target/debug/cooklang-import`).

### Batch Converter (Python)

Converts `.recipe` files to `.cook` using the OpenAI Batch API with a fine-tuned model.

```sh
pip install -r requirements.txt

python batch_convert.py prepare   # Generate JSONL batch input
python batch_convert.py submit    # Upload and start batch job
python batch_convert.py collect   # Download results when complete
```

Requires `OPENAI_API_KEY` env var. Optionally set `OPENAI_MODEL` to override the default model.

## Recipe Format

### Input (`.recipe`)

```yaml
---
title: Recipe Name
description: ...
image: https://...
servings: 2
time required: 30m
nutrition:
  calories: 500 kcal
  ...
---

2 unit Onion
1 tablespoon Olive Oil
...

• Dice the onion.
• Heat olive oil in a pan.
...
```

### Output (`.cook`)

```cooklang
Dice the @onion{2}. Heat @olive oil{1%tablespoon} in a #pan{}.
```

## License

The recipe content is sourced from HelloFresh and remains their intellectual property. This dataset is provided for research and educational purposes only.
