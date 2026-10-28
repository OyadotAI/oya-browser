"""Generate V9's Unicode 17 emoji sets from the pinned Test262 generated lists."""
import hashlib
import pathlib
import re
import sys

NAMES = ("Basic_Emoji", "Emoji_Keycap_Sequence", "RGI_Emoji", "RGI_Emoji_Flag_Sequence",
         "RGI_Emoji_Modifier_Sequence", "RGI_Emoji_Tag_Sequence", "RGI_Emoji_ZWJ_Sequence")


def decode(value):
    """Decode the scalar escapes used by Test262's generated strings."""
    pattern = r"\\u\{([0-9A-Fa-f]+)\}|\\u([0-9A-Fa-f]{4})|\\x([0-9A-Fa-f]{2})"
    result = re.sub(pattern, lambda m: chr(int(next(x for x in m.groups() if x), 16)), value)
    if not result or "\\" in result or any(0xD800 <= ord(c) <= 0xDFFF for c in result):
        raise ValueError("unexpected non-scalar or escaped source")
    return result


def extract(path):
    """Read positive strings only, and reject duplicates and unknown source formatting."""
    source = path.read_text(encoding="utf-8")
    if "Unicode v17.0.0" not in source:
        raise ValueError("unexpected Unicode version")
    body = source.split("matchStrings: [", 1)[1].split("],", 1)[0]
    words = [decode(word) for word in re.findall(r'"([^"\n]*)"', body)]
    if not words or len(words) != len(set(words)):
        raise ValueError("missing or duplicate entries")
    return words


def main():
    """Produce stable hexadecimal sequences and print source hashes for provenance."""
    root = pathlib.Path(sys.argv[1]) / "test/built-ins/RegExp/property-escapes/generated/strings"
    sets = {name: extract(root / (name + ".js")) for name in NAMES}
    combined = set().union(*(set(sets[name]) for name in NAMES if name != "RGI_Emoji"))
    if combined != set(sets["RGI_Emoji"]):
        raise ValueError("RGI_Emoji does not equal the union of its six properties")
    output = pathlib.Path(__file__).resolve().parents[1] / "src/v9/regexp/emoji17.txt"
    lines = ["# Unicode 17.0.0; generated from Test262, see THIRD_PARTY_NOTICES.md and UPSTREAM.md"]
    for name in NAMES:
        path = root / (name + ".js")
        print(name, len(sets[name]), hashlib.sha256(path.read_bytes()).hexdigest())
        lines.append("[" + name + "]")
        lines.extend(" ".join(f"{ord(c):X}" for c in word) for word in sets[name])
    output.write_text("\n".join(lines) + "\n", encoding="ascii")


if __name__ == "__main__":
    main()
