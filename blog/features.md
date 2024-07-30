---
title: Features
author: cstef
---

`uses.ink` comes with a lot of features out of the box. No configuration needed. You push and we render.

## Overview <!-- toc -->

## Metadata

You can add metadata to your markdown files by adding a YAML front matter block at the top of the file. This block must be the first thing in the file and must take the form of a valid YAML object. The metadata block is separated from the rest of the content by a line of three dashes.

Here is an example of a metadata block:

```yaml
---
title: My Awesome Post
author: John Doe
date: 2021-01-01
---
```

The following properties can be set in the metadata block:

- `title`: The title of the post.
- `description`: A short description of the post. This is used as the meta description for the post.
- `author`: The author of the post.
- `date`: The date the post was published. This should be in the format `YYYY-MM-DD` or `YYYY-MM-DD,HH:MM:SS`.
- `layout`: The layout to use when rendering the post. This can be `post` or `gallery`.
- `hideTop`: Whether to hide the top navigation bar.
- `readingTime`: Set to `false` to hide the reading time.
- `nav`: An object containing the navigation links for the post:
```yaml
nav:
  Home: /
  Blog: /blog        
``` 

## Callouts

You can add callouts by using the following syntax:

```markdown
> [!note]
> This is a note
```

> [!note]
> This is a note

The following callouts are available:

- `[!info]{:md}`
- `[!note]{:md}`
- `[!todo]{:md}`
- `[!abstract]{:md}` `[!summary]{:md}`  `[!tldr]{:md}`
- `[!tip]{:md}`  `[!hint]{:md}`  `[!important]{:md}`
- `[!success]{:md}`  `[!check]{:md}`  `[!done]{:md}`
- `[!question]{:md}`  `[!help]{:md}`  `[!faq]{:md}`
- `[!warning]{:md}`  `[!caution]{:md}`  `[!attention]{:md}`
- `[!failure]{:md}`  `[!fail]{:md}`  `[!missing]{:md}`
- `[!danger]{:md}`  `[!error]{:md}`
- `[!bug]{:md}`
- `[!example]{:md}`
- `[!quote]{:md}`  `[!cite]{:md}`

Callouts can also be foldable:

```markdown
> [!warning]- This is a note
> It can be folded
```

> [!warning]- This is a note
> It can be folded

## Code Blocks

Code blocks are rendered with syntax highlighting via [shiki](https://shiki.matsu.io/).
You can specify the language of the code block by adding the language name after the opening triple backticks.

~~~markdown
```python
print("Hello, World!")
```
~~~

```python
print("Hello, World!")
```

Inline code can be added by wrapping the code in backticks:

```markdown
What if `const a = 1;{:js}` ?
```

What if `const a = 1;{:js}` ?

### Line Numbers <!-- no-toc -->

You can add line numbers to code blocks by adding `showLineNumbers` to the code block:

~~~markdown
```python showLineNumbers
while True:
    print("Hello, World!")
```
~~~

```python showLineNumbers
while True:
    print("Hello, World!")
```

### Line Highlighting <!-- no-toc -->

You can highlight particular lines in a code block by specifying the line numbers after the language name:

~~~markdown
```python {1,3-5}
print("Hello, World!")
print("Hello, World!")
print("Hello, World!")
print("Hello, World!")
```
~~~

```python {1,3-5}
print("Hello, World!")
print("Hello, World!")
print("Hello, World!")
print("Hello, World!")
```

## Tables

Tables are rendered with GitHub Flavored Markdown:

```markdown
| Header 1 | Header 2 |
| -------- | -------- |
| Row 1    | Row 1    |
| Row 2    | Row 2    |
```

| Header 1 | Header 2 |
| -------- | -------- |
| Row 1    | Row 1    |
| Row 2    | Row 2    |

## Emojis

Shortcodes for emojis are supported:

```markdown
:smile:
```

:smile:

Emojis are rendered using [Twemoji](https://twemoji.twitter.com/).

## Tables of Contents

If a heading matching `/^.*<!--( ?)toc( ?)-->$/i{:regex}` is present in the markdown file, a table of contents will be generated.

In case you want to remove particular headings from the table of contents, you can add a `<!-- no-toc -->` comment after the heading:

```markdown
## Heading 1 <!-- no-toc -->
## Heading 2
```

An example of a table of contents is shown at the [beginning of this document](#table-of-contents).

## Math

Math is rendered using [typst](https://github.com/typst/typst)

```typ
$$
det(A) = sum_(k=1)^n (-1)^(k+1) a_1k det(A_1k) 
$$
```

$$
det(A) = sum_(k=1)^n (-1)^(k+1) a_1k det(A_1k)
$$

## Super- and Subscripts

You can add super- and subscripts using the `^` and `_` characters:

```markdown
H~2~O
21^st^ century
```

H~2~O
21^st^ century

## Footnotes

Footnotes can be added using the following syntax:

```markdown
Here is a footnote reference,[^1] and another.[^longnote]

[^1]: Here is the footnote.
[^longnote]: Here's one with multiple blocks.

    Subsequent paragraphs are indented to show that they belong to the previous footnote.
```

Here is a footnote reference,[^1] and another.[^longnote]

[^1]: Here is the footnote.
[^longnote]: Here's one with multiple blocks.

    Subsequent paragraphs are indented to show that they belong to the previous footnote.