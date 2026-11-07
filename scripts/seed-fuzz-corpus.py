#!/usr/bin/env python3
"""Copy checked-in fuzz seeds into corpus directories without replacing files."""

from pathlib import Path
import shutil
import sys


SEED_SOURCES = {
    "cafebabe_parse_class": "cafebabe_parse_class",
    "jbindgen_descriptors": "descriptors",
    "jbindgen_parse_class": "cafebabe_parse_class",
    "jni_mutf8": "jni_mutf8",
    "jni_signatures": "descriptors",
    "jni_string_roundtrip": "jni_string_roundtrip",
}


def seed_corpus(fuzz_root):
    for target, source in SEED_SOURCES.items():
        corpus = fuzz_root / "corpus" / target
        corpus.mkdir(parents=True, exist_ok=True)
        for seed in sorted((fuzz_root / "seeds" / source).iterdir()):
            if not seed.is_file():
                continue
            destination = corpus / seed.name
            try:
                # Exclusive creation also protects files created by another seed run.
                output = destination.open("xb")
            except FileExistsError:
                continue
            with output, seed.open("rb") as input_file:
                shutil.copyfileobj(input_file, output)


def main():
    fuzz_root = Path(__file__).resolve().parent.parent / "fuzz"
    try:
        seed_corpus(fuzz_root)
    except OSError as error:
        print(f"Unable to seed fuzz corpus: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
