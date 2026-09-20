#!/usr/bin/env python3
"""Small, dependency-free documentation checks; not product verification."""
import argparse
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


# Headings whose anchors this script must compute the way GitHub does, checked
# on every run against scripts/testdata/anchor-slugs.md. The em-dash rows are
# the ones that broke: GitHub drops the dash as punctuation and turns each
# surrounding space into a hyphen, so the anchor carries a doubled hyphen.
SLUG_CASES = {
    'J6 pilot, brian2: a brownfield repository, no model — **failed**':
        'j6-pilot-brian2-a-brownfield-repository-no-model--failed',
    'J6 pilot, Knowscroll-v2: decision memory, no model — **failed, then passed on the rerun**':
        'j6-pilot-knowscroll-v2-decision-memory-no-model--failed-then-passed-on-the-rerun',
    '`retrieval::COMPILER` and the golden digest': 'retrievalcompiler-and-the-golden-digest',
    '3. The budget, in code before the first call exists':
        '3-the-budget-in-code-before-the-first-call-exists',
    'What the pilots changed, and what changed back':
        'what-the-pilots-changed-and-what-changed-back',
    'A heading with [a link](../check_docs.py) in it': 'a-heading-with-a-link-in-it',
    'Punctuation: commas, colons; semicolons — and parentheses (like this)':
        'punctuation-commas-colons-semicolons--and-parentheses-like-this',
}


def slug(text):
    """The anchor GitHub gives a heading, by github-slugger's rule.

    Rendered text first: a link becomes its label and inline markers go. Then
    lowercase, drop everything that is not a letter, a digit, a space, a hyphen
    or an underscore -- an em dash included -- and turn each remaining space
    into a hyphen **without collapsing runs**, which is where the doubled
    hyphen in `no-model--failed` comes from.
    """
    text = re.sub(r'!?\[([^\]]*)\]\([^)]*\)', r'\1', text)
    text = text.replace('`', '').replace('*', '').replace('_', '')
    text = text.strip().lower()
    text = re.sub(r'[^\w\s-]', '', text, flags=re.UNICODE)
    return text.replace(' ', '-')


def anchors(content):
    """Every anchor a Markdown file offers, including GitHub's -1, -2 suffixes
    for headings that repeat."""
    seen = {}
    found = set()
    opened = None
    for line in content.splitlines():
        fence = re.match(r'^\s{0,3}(`{3,}|~{3,})(.*)$', line)
        if fence:
            marker, rest = fence.groups()
            if opened is None:
                opened = (marker[0], len(marker))
            elif marker[0] == opened[0] and len(marker) >= opened[1] and not rest.strip():
                opened = None
            continue
        if opened is not None:
            continue
        heading = re.match(r'^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$', line)
        if not heading:
            continue
        base = slug(heading.group(1))
        if not base:
            continue
        count = seen.get(base, 0)
        seen[base] = count + 1
        found.add(base if count == 0 else f'{base}-{count}')
    return found


def self_test(root):
    """The slug rule is checked here rather than remembered."""
    errors = []
    fixture = root / 'scripts' / 'testdata' / 'anchor-slugs.md'
    if not fixture.is_file():
        return [f'Missing slug fixture: {fixture.relative_to(root)}']
    for heading, expected in SLUG_CASES.items():
        got = slug(heading)
        if got != expected:
            errors.append(f'slug rule: {heading!r} -> {got!r}, expected {expected!r}')
    present = anchors(fixture.read_text(encoding='utf-8'))
    for expected in SLUG_CASES.values():
        if expected not in present:
            errors.append(f'slug fixture is missing a heading for {expected!r}')
    if 'duplicate-heading' not in present or 'duplicate-heading-1' not in present:
        errors.append('slug rule: a repeated heading must get GitHub\'s -1 suffix')
    return errors


def check(root, workspace=None):
    errors = self_test(root)
    checked = 0
    cross_skipped = 0
    fragments = 0
    # Anchors are wanted for files this run has not reached yet, so they are
    # read on demand and kept.
    anchor_cache = {}

    def offered(destination):
        key = destination.resolve()
        if key not in anchor_cache:
            try:
                anchor_cache[key] = anchors(destination.read_text(encoding='utf-8'))
            except OSError:
                anchor_cache[key] = None
        return anchor_cache[key]
    for name in ('README.md', 'AGENTS.md', 'CLAUDE.md', 'docs/README.md', 'docs/VERIFICATION.md'):
        if not (root / name).is_file():
            errors.append(f'Missing entrypoint: {name}')
    claude = root / 'CLAUDE.md'
    if claude.exists() and not re.search(r'^@AGENTS\.md\s*$', claude.read_text(), re.M):
        errors.append('CLAUDE.md must import the local AGENTS.md')
    paths = sorted(p for p in root.rglob('*.md') if not any(part in {'.git', '.worktrees', 'node_modules', 'target', '.venv', 'vendor'} for part in p.relative_to(root).parts))
    for path in paths:
        content = path.read_text(encoding='utf-8')
        rel = path.relative_to(root)
        # Match ordinary fenced Markdown blocks without treating prose as executable.
        opened = None
        prose = []
        for number, line in enumerate(content.splitlines(), 1):
            fence = re.match(r'^\s{0,3}(`{3,}|~{3,})(.*)$', line)
            if fence:
                marker, rest = fence.groups()
                if opened is None:
                    opened = (marker[0], len(marker), number)
                elif marker[0] == opened[0] and len(marker) >= opened[1] and not rest.strip():
                    opened = None
                continue
            if opened is None:
                prose.append(line)
        if opened:
            errors.append(f'{rel}:{opened[2]}: unclosed code fence')
        for match in re.finditer(r'\]\(([^)]+)\)', '\n'.join(prose)):
            target = match.group(1).strip()
            if target.startswith('<') and target.endswith('>'):
                target = target[1:-1]
            parts = urlsplit(target)
            if parts.scheme == 'mailto':
                continue
            if not parts.path:
                # A fragment alone: a link into this file's own headings.
                if parts.fragment:
                    fragments += 1
                    if parts.fragment not in offered(path):
                        errors.append(f'{rel}: no heading for anchor #{parts.fragment}')
                continue
            if parts.scheme in ('http', 'https'):
                cross = re.fullmatch(r'/Combraton/(combraton|pio|cbr|protocol|benchmarks)/blob/main/(.+)', parts.path)
                if parts.netloc == 'github.com' and cross:
                    if workspace:
                        destination = workspace / cross[1] / unquote(cross[2])
                    else:
                        cross_skipped += 1
                        continue
                else:
                    continue
            elif parts.scheme:
                errors.append(f'{rel}: unsupported local link scheme {parts.scheme}')
                continue
            else:
                destination = path.parent / unquote(parts.path)
                if parts.path.startswith('/'):
                    errors.append(f'{rel}: absolute machine path in link')
                    continue
            checked += 1
            if not destination.exists():
                errors.append(f'{rel}: missing link target {target}')
                continue
            # A fragment into another Markdown file, local or, with
            # --workspace, in a sibling clone.
            if parts.fragment and destination.suffix == '.md':
                available = offered(destination)
                if available is None:
                    continue
                fragments += 1
                if parts.fragment not in available:
                    errors.append(f'{rel}: no heading for anchor {target}')
        if re.search(r'/(?:Users|Volumes)/', content):
            errors.append(f'{rel}: private machine path in published Markdown')
    return errors, len(paths), checked, cross_skipped, fragments


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--workspace', type=Path, help='Optional parent containing all five clones, for cross-repository file links')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    errors, files, links, skipped, fragments = check(root, args.workspace.resolve() if args.workspace else None)
    for error in errors:
        print(error, file=sys.stderr)
    print(f'{root.name}: {files} Markdown files, {links} file links, {fragments} heading anchors, {len(errors)} errors; {skipped} cross-repository links not checked')
    print('Checks entrypoints, local imports, ordinary file links, fences, and heading anchors in Markdown links by GitHub\'s slug rule, which is itself checked against scripts/testdata/anchor-slugs.md on every run. Skips vendor/, whose Markdown belongs to another repository and is verified by scripts/verify_pin.py instead. Does not validate remote URLs, anchors in files it cannot read, Mermaid syntax or product behavior.')
    return 1 if errors else 0


if __name__ == '__main__':
    raise SystemExit(main())
