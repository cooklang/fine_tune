# Converted Recipes (Training Data)

Recipe pairs for fine-tuning LLM models on Cooklang conversion.

Each recipe exists as two files:

- **`.recipe`** — raw structured text scraped from HelloFresh (YAML frontmatter with metadata, followed by ingredient list and cooking steps)
- **`.cook`** — the same recipe converted to [Cooklang](https://cooklang.org) format

## Countries

| Code | Country        | Pairs |
|------|----------------|-------|
| at   | Austria        | 11    |
| au   | Australia      | 10    |
| be   | Belgium        | 1,602 |
| de   | Germany        | 1,536 |
| es   | Spain          | 614   |
| fr   | France         | 681   |
| gb   | United Kingdom | 4,810 |
| ie   | Ireland        | 623   |
| it   | Italy          | 605   |
| nl   | Netherlands    | 1,687 |
| us   | United States  | 5,548 |

**Total: ~17,700 pairs**

## File Naming

Files are named using the HelloFresh recipe slug with its ID:

```
{recipe-slug}-{hellofresh-id}.recipe
{recipe-slug}-{hellofresh-id}.cook
```
