#!/usr/bin/env python3
"""Review and fix .cook files for conversion issues using Claude Haiku 4.5."""

import argparse
import json
import re
import sys
from pathlib import Path

import anthropic
from dotenv import load_dotenv
from langdetect import detect

load_dotenv()

RECIPES_DIR = Path("recipes")

LANGUAGE_MAP = {
    "en": "English", "es": "Spanish", "fr": "French", "de": "German",
    "it": "Italian", "pt": "Portuguese", "nl": "Dutch", "ru": "Russian",
    "ja": "Japanese", "ko": "Korean", "zh-cn": "Chinese", "zh-tw": "Chinese",
    "ar": "Arabic", "hi": "Hindi", "tr": "Turkish", "pl": "Polish",
    "sv": "Swedish", "da": "Danish", "no": "Norwegian", "fi": "Finnish",
    "cs": "Czech", "ro": "Romanian", "hu": "Hungarian", "el": "Greek",
    "th": "Thai", "vi": "Vietnamese", "id": "Indonesian", "ms": "Malay",
    "ca": "Catalan", "he": "Hebrew", "af": "Afrikaans",
}

ENGLISH_COUNTRIES = {"us", "gb", "au", "ie", "nz", "ca"}


def detect_language(text: str) -> str:
    try:
        code = detect(text)
        return LANGUAGE_MAP.get(code, "English")
    except Exception:
        return "English"


def get_country_code(path: Path) -> str:
    """Extract country code from path like recipes/de/file.cook."""
    parts = path.relative_to(RECIPES_DIR).parts
    return parts[0] if parts else ""


FRONTMATTER_RE = re.compile(r"^---\s*\n(.*?\n)---\s*\n", re.DOTALL)


def has_frontmatter(cook_text: str) -> bool:
    """Check if file already has refine_status in YAML frontmatter."""
    m = FRONTMATTER_RE.match(cook_text)
    if not m:
        return False
    return "refine_status" in m.group(1)


def strip_frontmatter(cook_text: str) -> str:
    """Strip existing YAML frontmatter from cook text."""
    m = FRONTMATTER_RE.match(cook_text)
    if not m:
        return cook_text
    return cook_text[m.end():].lstrip("\n")


def add_frontmatter(cook_body: str) -> str:
    """Prepend refine_status YAML frontmatter to cook body."""
    return "---\nrefine_status: llm_reviewed\n---\n\n" + cook_body


def local_checks(cook_text: str, recipe_text: str) -> list[dict]:
    """Fast regex-based checks that don't need an LLM."""
    issues = []
    lines = cook_text.split("\n")

    for i, line in enumerate(lines, 1):
        # Markup inside notes
        if line.startswith(">"):
            for marker, name in [("@", "ingredient"), ("#", "cookware"), ("~", "timer")]:
                if marker in line:
                    issues.append({
                        "line": i,
                        "type": "markup_in_note",
                        "message": f"{name} markup ({marker}) found inside a note line",
                        "text": line.strip(),
                    })

        # Space between } and ( for prep instructions — only for @ingredients
        if re.search(r"@[^@#~]*\}\s+\(", line):
            issues.append({
                "line": i,
                "type": "space_before_prep",
                "message": "Space between } and ( — prep instructions should attach directly: }(prep)",
                "text": line.strip(),
            })

        # Tilde used for approximation (not timer)
        if re.search(r"~\d+\s*(g|kg|ml|l|cm|mm|oz|lb)\b", line, re.IGNORECASE):
            issues.append({
                "line": i,
                "type": "tilde_approximation",
                "message": "Tilde (~) used for approximation instead of 'about'",
                "text": line.strip(),
            })

        # Deprecated >> metadata syntax
        if re.match(r"^>>", line):
            issues.append({
                "line": i,
                "type": "deprecated_metadata",
                "message": "Deprecated >> metadata syntax found — use YAML frontmatter instead",
                "text": line.strip(),
            })

        # Missing {} after @ — detect @ingredient without braces
        # In Cooklang, ingredient name runs from @ to {, so we check if there's
        # a { before the next @, #, ~ marker or end of text segment
        for match in re.finditer(r"@\??", line):
            start = match.end()
            # Find what comes next: look for { before next marker or sentence end
            rest = line[start:]
            # Find position of next { and next marker (@, #, ~)
            brace_pos = rest.find("{")
            next_marker = len(rest)
            for marker_match in re.finditer(r"[@#~]", rest):
                next_marker = marker_match.start()
                break
            if brace_pos == -1 or brace_pos > next_marker:
                # No brace found before next marker — missing braces
                # Extract the ingredient name (up to next marker or punctuation)
                name_match = re.match(r"([\w]+)", rest)
                if name_match and len(name_match.group(1)) > 1:
                    issues.append({
                        "line": i,
                        "type": "missing_braces",
                        "message": f"Ingredient '@{name_match.group(1)}' missing curly braces",
                        "text": line.strip(),
                    })

    # Title/header on first line
    first_line = lines[0].strip() if lines else ""
    if first_line.startswith("#") and not first_line.startswith("##"):
        # Could be markdown header (title) rather than cookware
        if not re.match(r"#\w", first_line):
            issues.append({
                "line": 1,
                "type": "title_header",
                "message": "Possible title/header on first line — cook files should start with recipe steps",
                "text": first_line,
            })

    return issues


REVIEW_AND_FIX_PROMPT = """\
You are a Cooklang format reviewer and fixer. Analyze the converted .cook file against the original .recipe source, identify issues, and return a corrected version.

<recipe>
{recipe}
</recipe>

<cook>
{cook}
</cook>

The recipe language is: {language}
The country code is: {country}

## Cooklang Syntax Rules

INGREDIENTS: Use @ symbol. Always close with curly braces.
- Single-word: @salt{{}}
- Multi-word: @ground black pepper{{}}
- With quantity: @potato{{2}}
- With quantity and unit: @bacon strips{{1%kg}} or @syrup{{1/2%tbsp}}
- With preparation: @onion{{1}}(peeled and finely chopped) — NO space between }}(
- Optional: @?hash browns{{3-4}}
- IMPORTANT: keep ingredients in {language}. Do not translate.

COOKWARE: Use # symbol. Always close with curly braces. Only tag FIRST mention.
- Single-word: #pot{{}}
- Multi-word: #baking sheet{{}}
- Include size/descriptors as part of name: #small saucepan{{}}
- Do NOT tag common items (bowls, plates, knives, spoons, forks, cutting boards).

TIMERS: Use ~ symbol. Format: ~{{number%units}} or ~name{{number%units}}.
- Basic: ~{{25%minutes}}
- Named: ~eggs{{3%minutes}}
- Ranges: ~{{10-15%minutes}}
- Convert complex durations to single unit: "1 hour 45 minutes" → ~{{105%minutes}}
- ONLY use ~ for actual timers. Replace tilde approximations with "about".

STEPS: Each paragraph is a step, separated by an empty line.

SECTIONS: == Section Name ==

NOTES: Use > at start of line. Do NOT use @, #, ~ inside notes.

METADATA: Do NOT include any >> metadata lines. Do NOT include YAML frontmatter.

## Check for these issues (report ONLY real problems):

1. **translated_ingredient**: Ingredient names translated to English when the recipe is non-English. MOST IMPORTANT check.
2. **missing_ingredient**: Ingredients from .recipe not marked with @ in .cook (only if mentioned in text but not marked up).
3. **cookware_tagged_twice**: Cookware tagged with # more than once.
4. **temperature_as_timer**: Temperature incorrectly marked as timer with ~.
5. **wrong_quantity**: Ingredient quantity doesn't match .recipe.
6. **invented_ingredient**: @ingredient not in .recipe ingredient list at all.
7. **deprecated_metadata**: >> metadata syntax found — remove it.
8. **markup_in_note**: @, #, or ~ used inside a > note line — remove markup from notes.
9. **space_before_prep**: Space between }} and ( — should be }}(prep) with no space.
10. **tilde_approximation**: ~ used for approximation instead of "about".
11. **missing_braces**: Ingredient after @ missing curly braces.
12. **title_header**: Title/header on first line — cook files should start with recipe steps.

## Response Format

Respond with ONLY a JSON object (no other text):
{{
  "issues": [
    {{"type": "issue_type", "message": "brief description", "text": "short snippet (max 80 chars)"}}
  ],
  "fixed_cook": "the corrected .cook file content with all issues fixed"
}}

If no issues found, return empty issues array but STILL return the cook content as fixed_cook.

IMPORTANT: Do not report as issues:
- Salt/pepper/oil with generic quantities
- Minor wording differences preserving meaning
- Common kitchen items (bowls, plates, etc.) — these SHOULD NOT be tagged
- Unit format differences (e.g., "cucharadas" vs "cucharada(s)")
- Quantities intentionally split across steps

Keep all JSON string values on a single line. Use \\n for newlines inside fixed_cook."""


def llm_review(cook_text: str, recipe_text: str, language: str, country: str) -> tuple[list[dict], str | None]:
    """Use Claude Haiku to check for semantic issues and return fixed cook content.

    Returns (issues, fixed_cook). fixed_cook is None only on parse failure.
    """
    client = anthropic.Anthropic()

    prompt = REVIEW_AND_FIX_PROMPT.format(
        recipe=recipe_text,
        cook=cook_text,
        language=language,
        country=country,
    )

    response = client.messages.create(
        model="claude-haiku-4-5-20251001",
        max_tokens=8000,
        messages=[{"role": "user", "content": prompt}],
        temperature=0,
    )

    text = response.content[0].text.strip()

    # Extract JSON from response (handle markdown code blocks)
    if text.startswith("```"):
        text = re.sub(r"^```\w*\n?", "", text)
        text = re.sub(r"\n?```$", "", text)
        text = text.strip()

    try:
        result = json.loads(text)
        if not isinstance(result, dict):
            return [], None
        issues = result.get("issues", [])
        if not isinstance(issues, list):
            issues = []
        fixed_cook = result.get("fixed_cook")
        if isinstance(fixed_cook, str):
            # LLM uses \n literals for newlines in JSON strings
            fixed_cook = fixed_cook.strip()
        else:
            fixed_cook = None
        return issues, fixed_cook
    except json.JSONDecodeError:
        print(f"  Warning: Could not parse LLM response: {text[:200]}", file=sys.stderr)
        return [], None


def review_file(cook_path: Path, skip_llm: bool = False, check_only: bool = False) -> tuple[list[dict], str]:
    """Review a single .cook file, optionally fix it, and return (issues, status).

    status is one of: "fixed", "unchanged", "check_only", "local_only"
    """
    recipe_path = cook_path.with_suffix(".recipe")

    if not recipe_path.exists():
        return [{"type": "missing_recipe", "message": f"No .recipe file found for {cook_path.name}"}], "unchanged"

    cook_text = cook_path.read_text(encoding="utf-8", errors="replace")
    recipe_text = recipe_path.read_text(encoding="utf-8", errors="replace")

    # Strip frontmatter before checking
    cook_body = strip_frontmatter(cook_text)

    issues = local_checks(cook_body, recipe_text)

    if skip_llm:
        return issues, "local_only"

    country = get_country_code(cook_path)
    language = detect_language(recipe_text)
    llm_issues, fixed_cook = llm_review(cook_body, recipe_text, language, country)
    issues.extend(llm_issues)

    if check_only:
        return issues, "check_only"

    # Write fixed content with frontmatter
    if fixed_cook is not None:
        output = add_frontmatter(fixed_cook)
        status = "fixed" if fixed_cook != cook_body else "unchanged"
    else:
        # LLM parse failure — still add frontmatter to original
        output = add_frontmatter(cook_body)
        status = "unchanged"

    cook_path.write_text(output, encoding="utf-8")
    return issues, status


def main():
    parser = argparse.ArgumentParser(description="Review and fix .cook files for conversion issues")
    parser.add_argument("path", nargs="?", default=str(RECIPES_DIR),
                        help="Path to a .cook file, directory, or country code (e.g. 'de')")
    parser.add_argument("--local-only", action="store_true",
                        help="Only run local regex checks, skip LLM (no writes)")
    parser.add_argument("--check-only", action="store_true",
                        help="Report issues without fixing files")
    parser.add_argument("--force", action="store_true",
                        help="Reprocess files that already have refine_status frontmatter")
    parser.add_argument("--limit", type=int, default=0,
                        help="Max number of files to process (0 = all)")
    parser.add_argument("--json", action="store_true",
                        help="Output results as JSON")
    args = parser.parse_args()

    # Resolve path
    target = Path(args.path)
    if not target.exists():
        # Try as country code
        target = RECIPES_DIR / args.path
    if not target.exists():
        print(f"Error: {args.path} not found", file=sys.stderr)
        sys.exit(1)

    # Collect .cook files
    if target.is_file():
        cook_files = [target]
    else:
        cook_files = sorted(target.rglob("*.cook"))

    if not cook_files:
        print("No .cook files found.")
        return

    # Filter already-reviewed files unless --force
    if not args.force and not args.local_only:
        filtered = []
        skipped = 0
        for f in cook_files:
            text = f.read_text(encoding="utf-8", errors="replace")
            if has_frontmatter(text):
                skipped += 1
            else:
                filtered.append(f)
        if skipped and not args.json:
            print(f"Skipping {skipped} already-reviewed file(s). Use --force to reprocess.")
        cook_files = filtered

    if args.limit > 0:
        cook_files = cook_files[:args.limit]

    if not cook_files:
        print("No files to process.")
        return

    total = len(cook_files)
    files_with_issues = 0
    total_issues = 0
    files_fixed = 0
    files_unchanged = 0
    all_results = {}

    skip_llm = args.local_only
    check_only = args.check_only or args.local_only

    for idx, cook_path in enumerate(cook_files, 1):
        rel = cook_path.relative_to(RECIPES_DIR) if cook_path.is_relative_to(RECIPES_DIR) else cook_path
        if not args.json:
            print(f"[{idx}/{total}] {rel}", end="", flush=True)

        issues, status = review_file(cook_path, skip_llm=skip_llm, check_only=check_only)

        if status == "fixed":
            files_fixed += 1
        elif status == "unchanged":
            files_unchanged += 1

        if issues:
            files_with_issues += 1
            total_issues += len(issues)

            if args.json:
                all_results[str(rel)] = issues
            else:
                status_tag = f" [{status}]" if not check_only else ""
                print(f"  — {len(issues)} issue(s){status_tag}")
                for issue in issues:
                    line = issue.get("line", "")
                    line_str = f"L{line} " if line else ""
                    print(f"  {line_str}[{issue['type']}] {issue['message']}")
                    if "text" in issue:
                        print(f"    {issue['text']}")
        else:
            if not args.json:
                status_tag = f" [{status}]" if not check_only else ""
                print(f"  OK{status_tag}")

    if args.json:
        print(json.dumps(all_results, indent=2, ensure_ascii=False))
    else:
        print(f"\n{'='*60}")
        summary = f"Processed: {total}  Issues: {total_issues}  Files with issues: {files_with_issues}"
        if not check_only:
            summary += f"  Fixed: {files_fixed}  Unchanged: {files_unchanged}"
        print(summary)


if __name__ == "__main__":
    main()
