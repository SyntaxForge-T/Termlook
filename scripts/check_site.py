#!/usr/bin/env python3
"""Check a built site directory: local links, anchors, images and page metadata.

Usage: python3 -I scripts/check_site.py _site

Only local references are checked, so the script needs no network access.
Exits with status 1 and lists every problem when something is wrong.
"""

import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

REQUIRED_META = ("description", "viewport")


class PageParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.refs = []  # (attribute, value)
        self.meta = set()
        self.title = ""
        self.has_h1 = 0
        self.lang = None
        self.in_title = False
        self.missing_alt = 0

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            self.ids.add(a["id"])
        if tag == "html":
            self.lang = a.get("lang")
        elif tag == "title":
            self.in_title = True
        elif tag == "h1":
            self.has_h1 += 1
        elif tag == "meta" and a.get("name"):
            self.meta.add(a["name"])
        elif tag == "base":
            return  # 404.html sets <base> so it works from any URL depth
        elif tag == "img" and "alt" not in a:
            self.missing_alt += 1
        for attr in ("href", "src"):
            if attr in a and a[attr]:
                self.refs.append((attr, a[attr]))

    def handle_endtag(self, tag):
        if tag == "title":
            self.in_title = False

    def handle_data(self, data):
        if self.in_title:
            self.title += data


def main(argv):
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    root = Path(argv[1]).resolve()
    if not root.is_dir():
        print(f"{root} is not a directory", file=sys.stderr)
        return 2

    pages = {}
    for path in sorted(root.rglob("*.html")):
        parser = PageParser()
        parser.feed(path.read_text(encoding="utf-8"))
        pages[path] = parser

    errors = []
    for required in ("index.html", "404.html", "robots.txt", "sitemap.xml", "favicon.svg"):
        if not (root / required).is_file():
            errors.append(f"missing required file: {required}")

    for path, page in pages.items():
        name = path.relative_to(root)
        if not page.title.strip():
            errors.append(f"{name}: empty <title>")
        if page.has_h1 != 1:
            errors.append(f"{name}: expected exactly one <h1>, found {page.has_h1}")
        if not page.lang:
            errors.append(f"{name}: <html> has no lang attribute")
        for meta in REQUIRED_META:
            if meta not in page.meta:
                errors.append(f"{name}: missing <meta name=\"{meta}\">")
        if page.missing_alt:
            errors.append(f"{name}: {page.missing_alt} <img> without alt text")

        for attr, value in page.refs:
            parts = urlsplit(value)
            if parts.scheme in ("http", "https", "mailto", "tel", "data", "javascript"):
                continue
            if value.startswith("//"):
                errors.append(f"{name}: protocol-relative URL {value!r}")
                continue
            if value.startswith("/"):
                errors.append(f"{name}: root-relative {attr}={value!r} breaks under /Termlook/")
                continue
            target = path if not parts.path else (path.parent / unquote(parts.path))
            target = target.resolve()
            if root not in target.parents and target != root:
                errors.append(f"{name}: {attr}={value!r} escapes the site")
                continue
            if target.is_dir():
                target = target / "index.html"
            if not target.is_file():
                errors.append(f"{name}: broken {attr}={value!r}")
                continue
            if parts.fragment and target.suffix == ".html":
                other = pages.get(target)
                if other is None or parts.fragment not in other.ids:
                    errors.append(f"{name}: missing anchor in {attr}={value!r}")

    if errors:
        print(f"{len(errors)} problem(s) found:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print(f"Checked {len(pages)} page(s): no problems found")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
