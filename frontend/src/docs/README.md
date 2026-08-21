# Documentation

These Markdown files are the in-app **Documentation** at `/documentation`. Anyone can
contribute — add or edit a file and it shows up on its own.

## How it's organised

```
frontend/src/docs/
  01-getting-started/        ← a category (folder)
    01-overview.md           ← a page
    02-connecting-your-character.md
  02-signatures/
    ...
```

- **Folders are categories**, **files are pages**.
- The `NN-` prefix only controls **ordering** in the sidebar. It is stripped from the URL.
- The name minus the prefix becomes the **slug**, so
  `01-getting-started/02-connecting-your-character.md` is served at
  `/documentation/getting-started/connecting-your-character`.

## Frontmatter

Each page opens with a small YAML block:

```markdown
---
title: Connecting your character
---

# Connecting your character

Body content in **Markdown**…
```

- `title` — the label in the sidebar and the browser tab. Optional; the first `# Heading`
  is used when it is missing.
- `category` — an override for the category label. Optional; the folder name, humanised,
  is used otherwise.

## Writing content

- Write for somebody flying, not somebody reading the source. What the thing does for
  them, and what happens if they get it wrong.
- Standard Markdown: headings, lists, tables, code and blockquotes all render.
- Link between pages with absolute paths, e.g.
  `[Mass](/documentation/connections/mass)`.
- Raw HTML is not rendered. Stick to Markdown.

## Adding a page

1. Drop `NN-your-page.md` into the right folder, or make a new `NN-category/`.
2. Give it a `title`.
3. Write. The sidebar, the ordering and the prev/next links follow on their own.
