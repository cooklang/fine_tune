#!/usr/bin/env python3
"""Batch convert .recipe files to Cooklang format using OpenAI Batch API."""

import argparse
import json
import os
import sys
from pathlib import Path

from langdetect import detect
from openai import OpenAI

RECIPES_DIR = Path("recipes")
BATCH_INPUT_FILE = Path("batch_input.jsonl")
BATCH_STATE_FILE = Path("batch_state.json")
PROMPT_TEMPLATE_PATH = Path("../cooklang-import/src/converters/prompt.txt")

LANGUAGE_MAP = {
    "en": "English", "es": "Spanish", "fr": "French", "de": "German",
    "it": "Italian", "pt": "Portuguese", "nl": "Dutch", "ru": "Russian",
    "ja": "Japanese", "ko": "Korean", "zh-cn": "Chinese", "zh-tw": "Chinese",
    "ar": "Arabic", "hi": "Hindi", "tr": "Turkish", "pl": "Polish",
    "sv": "Swedish", "da": "Danish", "no": "Norwegian", "fi": "Finnish",
    "cs": "Czech", "ro": "Romanian", "hu": "Hungarian", "el": "Greek",
    "th": "Thai", "vi": "Vietnamese", "id": "Indonesian", "ms": "Malay",
    "uk": "Ukrainian", "bg": "Bulgarian", "hr": "Croatian", "sk": "Slovak",
    "sl": "Slovenian", "sr": "Serbian", "ca": "Catalan", "he": "Hebrew",
    "fa": "Persian", "lt": "Lithuanian", "lv": "Latvian", "et": "Estonian",
    "tl": "Tagalog", "sw": "Swahili", "af": "Afrikaans",
}


def detect_language(text: str) -> str:
    try:
        code = detect(text)
        return LANGUAGE_MAP.get(code, "English")
    except Exception:
        return "English"


def load_prompt_template() -> str:
    if not PROMPT_TEMPLATE_PATH.exists():
        print(f"Error: Prompt template not found at {PROMPT_TEMPLATE_PATH}", file=sys.stderr)
        sys.exit(1)
    return PROMPT_TEMPLATE_PATH.read_text()


def inject_recipe(template: str, recipe_content: str, language: str) -> str:
    return template.replace("{{RECIPE}}", recipe_content).replace("{{LANGUAGE}}", language)


def get_model() -> str:
    return os.environ.get("OPENAI_MODEL", "ft:gpt-4.1-mini-2025-04-14:personal::D2a2pTzd")


def prepare():
    template = load_prompt_template()
    model = get_model()

    total = 0
    skipped = 0
    to_convert = 0

    with open(BATCH_INPUT_FILE, "w") as out:
        for recipe_path in sorted(RECIPES_DIR.rglob("*.recipe")):
            total += 1
            cook_path = recipe_path.with_suffix(".cook")
            if cook_path.exists():
                skipped += 1
                continue

            content = recipe_path.read_text(encoding="utf-8", errors="replace")
            language = detect_language(content)
            prompt = inject_recipe(template, content, language)
            custom_id = str(recipe_path.relative_to(RECIPES_DIR))

            line = json.dumps({
                "custom_id": custom_id,
                "method": "POST",
                "url": "/v1/chat/completions",
                "body": {
                    "model": model,
                    "messages": [{"role": "user", "content": prompt}],
                    "temperature": 0.9,
                    "max_tokens": 2000,
                },
            })
            out.write(line + "\n")
            to_convert += 1

    print(f"Total recipes:  {total}")
    print(f"Skipped (.cook exists): {skipped}")
    print(f"To convert:     {to_convert}")
    print(f"Output: {BATCH_INPUT_FILE}")


def submit():
    client = OpenAI()

    if not BATCH_INPUT_FILE.exists():
        print(f"Error: {BATCH_INPUT_FILE} not found. Run 'prepare' first.", file=sys.stderr)
        sys.exit(1)

    print("Uploading batch input file...")
    with open(BATCH_INPUT_FILE, "rb") as f:
        uploaded = client.files.create(file=f, purpose="batch")
    print(f"Uploaded file: {uploaded.id}")

    print("Creating batch...")
    batch = client.batches.create(
        input_file_id=uploaded.id,
        endpoint="/v1/chat/completions",
        completion_window="24h",
    )

    state = {"batch_id": batch.id, "input_file_id": uploaded.id}
    BATCH_STATE_FILE.write_text(json.dumps(state, indent=2))

    print(f"Batch ID: {batch.id}")
    print(f"Status:   {batch.status}")
    print(f"Saved to: {BATCH_STATE_FILE}")


def collect():
    client = OpenAI()

    if not BATCH_STATE_FILE.exists():
        print(f"Error: {BATCH_STATE_FILE} not found. Run 'submit' first.", file=sys.stderr)
        sys.exit(1)

    state = json.loads(BATCH_STATE_FILE.read_text())
    batch_id = state["batch_id"]

    batch = client.batches.retrieve(batch_id)
    print(f"Batch ID: {batch.id}")
    print(f"Status:   {batch.status}")

    if batch.request_counts:
        rc = batch.request_counts
        print(f"Progress: {rc.completed}/{rc.total} completed, {rc.failed} failed")

    if batch.status != "completed":
        if batch.status == "failed":
            print(f"Batch failed.", file=sys.stderr)
            if batch.errors and batch.errors.data:
                for err in batch.errors.data:
                    print(f"  Error: {err.message}", file=sys.stderr)
        else:
            print("Batch not yet completed. Run 'collect' again later.")
        return

    if not batch.output_file_id:
        print("Error: No output file available.", file=sys.stderr)
        sys.exit(1)

    print("Downloading results...")
    content = client.files.content(batch.output_file_id)
    results = content.text

    success = 0
    errors = 0
    for line in results.strip().split("\n"):
        if not line:
            continue
        result = json.loads(line)
        custom_id = result["custom_id"]
        recipe_path = RECIPES_DIR / custom_id
        cook_path = recipe_path.with_suffix(".cook")

        response = result.get("response")
        if not response or response.get("status_code") != 200:
            error_msg = result.get("error", {}).get("message", "unknown error")
            print(f"  FAIL: {custom_id} - {error_msg}")
            errors += 1
            continue

        try:
            body = response["body"]
            text = body["choices"][0]["message"]["content"]
            cook_path.write_text(text, encoding="utf-8")
            success += 1
        except (KeyError, IndexError) as e:
            print(f"  FAIL: {custom_id} - {e}")
            errors += 1

    print(f"\nDone: {success} converted, {errors} errors")


def main():
    parser = argparse.ArgumentParser(description="Batch convert recipes to Cooklang")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("prepare", help="Generate JSONL batch input")
    sub.add_parser("submit", help="Upload and create batch job")
    sub.add_parser("collect", help="Check status and download results")

    args = parser.parse_args()

    if args.command == "prepare":
        prepare()
    elif args.command == "submit":
        submit()
    elif args.command == "collect":
        collect()


if __name__ == "__main__":
    main()
