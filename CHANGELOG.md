## v0.2.0 (2024-07-23)

### Fix

- ci included too many instances

## v0.1.23 (2024-07-23)

### Fix

- wrong value in ci

## v0.1.22 (2024-07-23)

### Feat

- allow `no-anchor` id for titles without the `#` link

### Fix

- demo link

### Refactor

- do not show build date, show version instead

## v0.1.21 (2024-07-23)

## v0.1.20 (2024-07-23)

### Fix

- remove .git from dockerignore

## v0.1.19 (2024-07-23)

### Fix

- add .git to docker

## v0.1.18 (2024-07-23)

### Fix

- add git to docker image

## v0.1.17 (2024-07-23)

### Feat

- add heading anchors with rehype-slug
- add go back to top + build information

### Fix

- Add a home link in error page

### Refactor

- add proper error page

## v0.1.16 (2024-07-23)

### Fix

- style inline code blocks
- code blocks titles
- image centered
- add top padding for code blocks

## v0.1.15 (2024-07-23)

### Fix

- allow cache to fail

## v0.1.14 (2024-07-23)

### Feat

- add opengraph stuff

### Fix

- do not set line-height to 2

## v0.1.13 (2024-07-23)

### Feat

- add support for config at `uses.ink.json`
- add readTime to frontmatter to allow disabling reading time on posts

### Fix

- reposition diff signs
- correctly fetch config
- use dirname for config searching

### Refactor

- switch to ioredis

## v0.1.12 (2024-07-22)

### Fix

- diff signs not positioned correctly

### Refactor

- make title bigger and allow lg-prose on xl screens

## v0.1.11 (2024-07-22)

### Feat

- resolve title to file name in case none is found
- add icons to titled code blocks
- switch to rehype-pretty-code for syntax highlighting
- add rehype-slug

### Fix

- restrain title search to h3
- adapt wrap toggle for pretty-code
- check if content is iterable before resolving title
- re-add default allowed attributes

## v0.1.10 (2024-07-21)

### Feat

- add word wrap toggle to pre

### Fix

- root path wasn't working anymore for some reason

### Refactor

- move shiki highlighting to css variables for external access

## v0.1.9 (2024-07-21)

### Feat

- allow style and class attributes

### Fix

- ignore repo and branch when no owner is specified (root domain)

### Refactor

- move multiple parts to components

## v0.1.8 (2024-07-21)

### Fix

- add link to the return type of fetchData

## v0.1.7 (2024-07-21)

### Feat

- add informational tooltips to header

## v0.1.6 (2024-07-21)

### Fix

- undefined appearing as branch name

## v0.1.5 (2024-07-21)

### Fix

- correct overall theme

## v0.1.4 (2024-07-21)

## v0.1.3 (2024-07-21)

## v0.1.2 (2024-07-21)

### Refactor

- use v-prefixed tag

## 0.1.1 (2024-07-21)

### Feat

- add docker support
- add footer
- add support for raw HTML (sanitized)
- add dev tools constants
- add hideTop to frontmatter
- add default page
- add blog header

### Fix

- tweak theme select button dropdown border color
- tweak positioning for diff signs
- display correct icon
- specify in url resolvers
- fix branch specification
- fix navigation not working on default repo
- re-add default repository
- only pick up branch in the repo part of the url
- patch sanitization to allow frontmatter data to pass through
- add x-pad for body
- update cache time
- check if meta is some before setting

### Refactor

- remove useless root div and wrap everything in article
