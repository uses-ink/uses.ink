---
title: FAQ
author: cstef
---

## Can I use my own domain ?

At the moment, you can't use your own domain. This is a feature that is planned for the future.

## How can I add a new blog post ?

Just create a new markdown file in the repository and it will be automatically be available on your blog. You will however need to add a link to the new post somewhere in your blog for people to be able to access it.

The classic way to do this is to add a link to your new file in the `README.md`.

## Can I use custom CSS ?

You kind of can. Elements can be styled using the `style` attribute in the markdown files. This is not recommended as it can make the blog look inconsistent and hard to maintain.

```markdown
<div style="background-color: #f00; color: #fff; padding: 1em; border-radius: 5px;">
  This is a styled div.
</div>
```

<div style="background-color: #f00; color: #fff; padding: 1em; border-radius: 5px;">
  This is a styled div.
</div>

## Can I use custom JavaScript ?

No. JavaScript is disabled for obvious security reasons.

## I found a bug, what should I do ?

Please report it by creating an issue on the [GitHub repository](https://github.com/uses-ink/uses.ink/issues/new).

## Where is this hosted ?

This instance of [uses.ink](https://uses.ink) is hosted on my own server using the [Docker image](https://ghcr.io/uses-ink/uses.ink) provided by the project.