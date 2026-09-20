# Anchor slug fixture

Headings whose GitHub anchors `scripts/check_docs.py` must compute correctly. It
is checked on every run, not on request: fragment validation is only a mechanism
if it cannot be skipped, and the rule it encodes was kept by memory four times
before it was written down.

The em-dash case is here because it was the one that broke. GitHub removes the
dash as punctuation and leaves the spaces either side, each of which becomes a
hyphen, so the anchor carries a doubled hyphen that nobody types by hand.

## J6 pilot, brian2: a brownfield repository, no model — **failed**

## J6 pilot, Knowscroll-v2: decision memory, no model — **failed, then passed on the rerun**

## `retrieval::COMPILER` and the golden digest

## 3. The budget, in code before the first call exists

## What the pilots changed, and what changed back

## A heading with [a link](../check_docs.py) in it

## Punctuation: commas, colons; semicolons — and parentheses (like this)

## Duplicate heading

## Duplicate heading
