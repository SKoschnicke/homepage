# Content Workflow

This document describes how content is authored and managed in this Hugo site.

## Overview

Content is authored in **Emacs Org-mode** and exported to Markdown using **ox-hugo**. The Markdown files in `/content/` are auto-generated and should never be edited directly.

```
content-org/all-pages.org  →  ox-hugo export  →  content/*.md  →  Hugo build
```

## Source File Structure

All content lives in a single file: `content-org/all-pages.org`

```org
#+hugo_base_dir: ../

* Pages
:PROPERTIES:
:EXPORT_HUGO_SECTION: /
:END:

** Home                                                            :ATTACH:
:PROPERTIES:
:EXPORT_FILE_NAME: _index
:ID:       9ef0075f-3e73-49a7-ab6d-ade5b436f7fa
:END:

Welcome content...

** About
:PROPERTIES:
:EXPORT_FILE_NAME: about
:ID:       1cf09891-23bf-4f31-a572-750cc9453778
:END:

About page content...

* Posts

** DONE Magic Wormhole                                           :software:
:PROPERTIES:
:EXPORT_FILE_NAME: magic-wormhole
:ID:       d25b0e33-8bd4-44cc-9249-dcfd4eff5b1a
:END:

Post content...
```

## Key Properties

Each heading that represents a page/post uses these Org-mode properties:

| Property | Required | Description |
|----------|----------|-------------|
| `EXPORT_FILE_NAME` | Yes | Output filename (without `.md`) |
| `EXPORT_HUGO_SECTION` | No | Target directory under `/content/` |
| `ID` | No | Unique identifier for cross-references |

## Tags and Categories

Tags are specified after the heading title using Org-mode tag syntax:

```org
** DONE Post Title                                    :tag1:tag2:tag3:
```

Common tags used:
- `:software:` - Software-related posts
- `:personal:` - Personal posts
- `:ATTACH:` - Indicates the heading has attachments (images)

## Status Keywords

The `DONE` keyword before a heading title marks it as published:

```org
** DONE Published Post Title                          :tag:
** TODO Draft Post Title                              :tag:
```

Hugo only exports headings marked as `DONE`.

## Images and Attachments

Images can be attached to headings using Org-mode's attachment system:

```org
** DONE Post With Image                                          :ATTACH:
:PROPERTIES:
:EXPORT_FILE_NAME: post-slug
:ID:       abc123
:END:

#+DOWNLOADED: screenshot @ 2026-01-20 19:52:46
#+ATTR_HTML: :class homepage-image
[[attachment:2026-01-20_19-52-46_screenshot.png]]

Post content...
```

Attachments are stored in `content-org/data/{id-prefix}/` and exported to `static/ox-hugo/`.

## Exporting Content

### In Emacs

1. Open `content-org/all-pages.org`
2. Make your edits
3. Export using ox-hugo:
   - `C-c C-e H H` - Export current subtree
   - `C-c C-e H A` - Export all subtrees

### Generated Output

For a heading like:
```org
** DONE Magic Wormhole                                           :software:
:PROPERTIES:
:EXPORT_FILE_NAME: magic-wormhole
:END:
```

ox-hugo generates `content/posts/magic-wormhole.md`:
```markdown
+++
title = "Magic Wormhole"
tags = ["software"]
draft = false
+++

Post content here...
```

## Section Organization

The `EXPORT_HUGO_SECTION` property controls where content is placed:

| Section | Property Value | Output Directory |
|---------|----------------|------------------|
| Pages | `/` | `content/` |
| Posts | `posts` | `content/posts/` |

Top-level headings (`* Pages`, `* Posts`) set the section for all children via inherited properties.

## Adding New Content

### New Blog Post

1. Add under `* Posts`:
   ```org
   ** DONE Your Post Title                                        :your-tag:
   :PROPERTIES:
   :EXPORT_FILE_NAME: your-post-slug
   :END:

   Your content here. Use standard Org-mode formatting:

   - Lists work
   - *Bold* and /italic/
   - [[https://example.com][Links]]

   #+begin_src python
   # Code blocks work
   print("Hello, world!")
   #+end_src
   ```

2. Export with `C-c C-e H H`
3. Hugo picks up the new file automatically

### New Page

1. Add under `* Pages`:
   ```org
   ** Your Page Title
   :PROPERTIES:
   :EXPORT_FILE_NAME: page-slug
   :END:

   Page content...
   ```

2. Export and add to navigation if needed (in `hugo.toml`)

## Org-mode Formatting Reference

| Org-mode | Renders as |
|----------|------------|
| `*bold*` | **bold** |
| `/italic/` | *italic* |
| `=code=` | `code` |
| `~verbatim~` | verbatim |
| `[[url][text]]` | [text](url) |
| `#+begin_src lang` | Code block |
| `#+begin_quote` | Blockquote |
| `- item` | Bullet list |
| `1. item` | Numbered list |

## Troubleshooting

### Content not appearing

- Check that the heading has `DONE` status (not `TODO`)
- Verify `EXPORT_FILE_NAME` is set
- Make sure you exported after making changes

### Images not showing

- Use `:ATTACH:` tag on the heading
- Ensure the attachment path is correct
- Check that the image file exists in `content-org/data/` or `static/ox-hugo/`

### Wrong section

- Check `EXPORT_HUGO_SECTION` on the heading or parent heading
- Posts should be under `* Posts`, pages under `* Pages`
