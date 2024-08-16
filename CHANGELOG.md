## v0.5.3 (2024-08-17)

### Fix

- try all the readme patterns
- weird list layout on firefox
- rework error handling for page not found

### Refactor

- fetch the tree instead of trying each readme
- remove search functionality

## v0.5.2 (2024-08-15)

### Feat

- implement caching for smaller portions of the rendering

### Fix

- **deps**: update dependency tailwind-merge to v2.5.2 (#48)
- **deps**: update dependency ts-pattern to v5.3.1 (#42)
- **deps**: update dependency tailwind-merge to v2.5.2 (#43)
- **deps**: update shiki monorepo to v1.13.0 (#47)
- add separator for auto-generated label
- nicer error message for repo not found in auto-generated readme
- auto-generated readme should generate correctly
- **deps**: update dependency tailwind-merge to v2.5.0 (#41)
- add default repo in page name
- **deps**: update dependency lucide-react to ^0.427.0 (#39)

### Refactor

- move d2.wasm.br out of the public directory because this is used on the server

## v0.5.1 (2024-08-09)

### Feat

- add `copy` meta for codeblocks + `defaultCopy` config

### Fix

- **deps**: update dependency lucide-react to ^0.426.0 (#38)

### Refactor

- disable render cache for dev mode
- do not parse matter before checking cache

## v0.4.3 (2024-08-08)

### Feat

- add render caching
- add pikchr diagrams rendering

### Fix

- correctly set loggers levels in dev

### Refactor

- switch to consola for logging

## v0.4.2 (2024-08-08)

### Fix

- search the config file correctly

### Refactor

- switch to cli version of d2

## v0.4.1 (2024-08-07)

### Fix

- only instanciate the wasm module once

## v0.4.0 (2024-08-07)

### Feat

- add d2 support for diagrams

### Fix

- **deps**: update dependency cache-manager to v5.7.6 (#33)
- use latest version of chromium

### Refactor

- use brotli-compressed wasm

## v0.3.15 (2024-08-05)

### Feat

- support for mermaid diagrams
- add support for user-config at `<user>/<user>`

### Fix

- **deps**: update dependency lucide-react to ^0.424.0 (#31)
- **deps**: update dependency lucide-react to ^0.419.0 (#29)
- **deps**: update shiki monorepo to v1.12.1 (#28)

## v0.3.14 (2024-08-02)

### Fix

- condition for dark/light images

## v0.3.13 (2024-08-02)

### Feat

- add support for`#gh-dark-mode-only`

### Fix

- do not restrict non-theme-restricted images
- move image to client component
- invert theme restriction method
- button styling matches the shiki background
- do not show the filename if it's a readme
- add little margin to the wrap text button

### Refactor

- use undefined instead of null

## v0.3.12 (2024-08-02)

### Feat

- switch to logging with pino

### Fix

- correct metadata url
- move metadata to head tags
- add metadataBase in metadata
- remove remaining console.log's

### Refactor

- delay config fetching a bit for faster metadata resolving
- metadata parsing and setting on the server

## v0.3.11 (2024-08-01)

### Feat

- add opengraph headers to posts

### Fix

- change blog to docs in dockerfile
- more consistent theme color for code blocks
- **deps**: update dependency lucide-react to ^0.418.0 (#27)

### Refactor

- more client code splitting
- move blog to docs
- split between client and server code
- modularize further
- move readme to component

## v0.3.10 (2024-07-31)

### Fix

- features links

## v0.3.9 (2024-07-31)

### Feat

- automatic readme generation
- add noHighlight to prevent highlighting
- support both typst and latex rendering for math

### Fix

- url again
- url for frontmatter doc
- hydration error on theme selector
- hacky way to correctly display badges
- correctly parse meta
- add offset to anchor scrolling

### Refactor

- throw errors on fail
- return earlier if filename is not supported
- move zod parsing to compile
- reorganise files + start adding git tree features

## v0.3.8 (2024-07-31)

### Fix

- access document on mount
- typst text color didn't match the current theme

### Refactor

- do not rely on react hooks for running as it is sync

## v0.3.7 (2024-07-30)

### Fix

- try to find a workaround for typst
- maybe we do need the patch after all

### Refactor

- don't need the patch anymore
- no need for the typst patch

## v0.3.6 (2024-07-30)

### Feat

- control table of contents with comments
- render math with typst instead of katex

### Fix

- **deps**: update dependency redis to v4.7.0 (#26)
- **deps**: update dependency cache-manager to v5.7.4 (#25)
- **deps**: update dependency lucide-react to ^0.417.0 (#24)
- hydration warning because of timezones

## v0.3.5 (2024-07-28)

### Feat

- add `<!-- no-toc -->` to bypass table of contents rendering

### Fix

- footnotes ids mismatch because of clobbering prevention
- don't use single-tilde gfm strikethrough
- padding for non-styled pre
- correct katex rendering
- add `nowrap` to inline code blocks
- correct regexp for skipping toc

## v0.3.4 (2024-07-28)

### Feat

- switch to rehype callouts for vitepress-styled callouts
- add emoji + twemoji support

### Fix

- **deps**: update shiki monorepo to v1.12.0 (#22)

### Refactor

- split css into their respective files

## v0.3.3 (2024-07-27)

### Fix

- add proper typing to layouts

## v0.3.2 (2024-07-27)

### Feat

- add callouts support
- add layouts + gallery layout
- add healthcheck for docker-compose

### Fix

- set debug_tree back to false
- remove backticks in inline highlighted code
- port copy to pretty-code
- roll back to remote image instead of local building

## v0.3.1 (2024-07-26)

### Feat

- add navbar support
- ability to select current match

### Fix

- remove redundant class
- fixed positioning for search
- **deps**: update dependency lucide-react to ^0.416.0
- **deps**: update shiki monorepo to v1.11.2

## v0.3.0 (2024-07-26)

### Feat

- add search bar for post content
- add table of contents support
- proper loading of client-side pages
- automatically set last updated for local content
- add link to commit/tag in footer tooltip

### Fix

- error page wasn't following theme
- shiki esm errors gone
- do not fetch config if content is local
- only include 8 first chars of commit hash

### Refactor

- prepare for an eventual migration to mdx

## v0.2.5 (2024-07-23)

## v0.2.4 (2024-07-23)

## v0.2.3 (2024-07-23)

## v0.2.2 (2024-07-23)

### Fix

- **deps**: update dependency lucide-react to ^0.414.0
- **deps**: update shiki monorepo to v1.11.1

## v0.2.1 (2024-07-23)

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
