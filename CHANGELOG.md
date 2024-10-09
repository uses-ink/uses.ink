## v0.7.0 (2024-10-10)

### Feat

- add headings + choose renderer
- add render time
- improve error page
- add support for fonts
- add support for themes + add some default themes
- improve not-found page
- better error handling
- make pre elements interactive
- add custom img components
- add footer
- switch to bun

### Fix

- postinstall script musl detection
- code title border color
- postinstall script to also work on x64 systems and remove the correct file based on the current system
- correctly import inter font
- git describe not working when no tag is present
- minor improvements/bugfixes
- save space in docker by removing typst-compiler for gnu and only keeping musl
- user-content hrefs
- minor improvements/fixes for themes
- make code blocks titles stick
- remove testing value
- responsive not-found
- correct url in not-found
- better mode toggling

### Refactor

- postinstall script refactored
- move configuration part out of constants
- start looking for alternative markdown renderers
- disable d2 rendering at the moment
- use `.glob()` for loading local files
- move markdown and typst to separate components

## v0.6.0 (2024-08-22)

### Feat

- typst rendering
- implement local rendering

### Fix

- load on client correctly
- add docker stuff

### Refactor

- switch to npm monorepo
