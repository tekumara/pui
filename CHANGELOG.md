# Changelog

## [0.1.1](https://github.com/tekumara/pui/compare/pui-v0.1.0...pui-v0.1.1) (2026-02-16)


### Features

* add Ctrl+A select all shortcut ([8607822](https://github.com/tekumara/pui/commit/8607822992f0ca5e0357bf798b03f7989dc23896))
* custom commands ([3d3ef06](https://github.com/tekumara/pui/commit/3d3ef06bae76f42f69326f7071b4b62b5d69d4e2))
* move to next line on space, wrapping around ([fb31847](https://github.com/tekumara/pui/commit/fb318476b7b67f030ee65103f7b9b9c378334f22))
* multi-select ([98e9730](https://github.com/tekumara/pui/commit/98e9730bb46c03918d0ac0d654caf0188b0f361c))
* sort by columns ([c40e39e](https://github.com/tekumara/pui/commit/c40e39e4e882f89139f021686d010c5cc98a110f))
* start stashed tasks ([8e6db2f](https://github.com/tekumara/pui/commit/8e6db2ff9e8e05874ba7c0c3e3e37d82f91fab35))


### Bug Fixes

* add select and logs keyboard shortcuts to the header ([f866940](https://github.com/tekumara/pui/commit/f866940e88a1102e297350aed8d0d23be3786237))
* after removing last task move to previous row ([eee3979](https://github.com/tekumara/pui/commit/eee3979edbe58ca7a055b5367521fc5900188dcf))
* allow tests to run without requiring stdin ([845ade5](https://github.com/tekumara/pui/commit/845ade5431807b2fc8febdf107908a7a2166416b))
* clippy warnings ([6b2bb5c](https://github.com/tekumara/pui/commit/6b2bb5c1164c6f0266bb99147eddcd6457fbe1d1))
* don't hang when viewing logs for stashed task ([5bd9c37](https://github.com/tekumara/pui/commit/5bd9c37044308d3759d4ed37956dbd1a8cde2907))
* Esc preserves selection when exiting filter ([d449ff8](https://github.com/tekumara/pui/commit/d449ff8db0e568a4e0416d3dfa3ef1807540796c))
* give Path column more space in task table ([6c93c5c](https://github.com/tekumara/pui/commit/6c93c5c4b9c4e2f55745c64bbda6dd9a130cb2bc))
* log viewer correctly scrolls to the end ([1be5f2b](https://github.com/tekumara/pui/commit/1be5f2b46a70f27156d79a90da7e7e0f856ca0ff))
* log viewer overscrolling and producing blank lines ([bb51968](https://github.com/tekumara/pui/commit/bb519681e4f42840536d19aef173397c3762e29d))
* make custom command tests cross-platform for Windows ([575c91c](https://github.com/tekumara/pui/commit/575c91c759db8712d6f2d12fc394605cc14c565e))
* move cursor to top-left before running custom command ([64f5ff5](https://github.com/tekumara/pui/commit/64f5ff5b5a0577a44abcf60fc49797f714c9c11a))
* PgDn/PgDn no longer scrolls back after returning from custom command ([6ed453c](https://github.com/tekumara/pui/commit/6ed453c78a64f9a67fdae7e2f9d9f025c80c28ec))
* removing running or paused task will do nothing ([0ce1cf3](https://github.com/tekumara/pui/commit/0ce1cf380e144aa0ed3b050000be4ad24c7829b2))
* render help modal scrollbar inside the border ([32cd793](https://github.com/tekumara/pui/commit/32cd79353ab046a9ab3e23b66eef1d4813da6635))
* scroll bar thumb in help modal now scrolls to the bottom ([f269437](https://github.com/tekumara/pui/commit/f26943760140262256d85b2a6540803d77a31ead))
* use %APPDATA%\pui\config.toml on Windows ([45ccbf8](https://github.com/tekumara/pui/commit/45ccbf88f628fc3c16276cca602cb578b21bedb1))
* **windows:** connect to pueued in platform agnostic way ([d4a65cb](https://github.com/tekumara/pui/commit/d4a65cbb6e89300407db524c38185caed1b797f0))


### Chores

* bump crossterm 0.29.0 ([7d60c7d](https://github.com/tekumara/pui/commit/7d60c7dfd8785ac4da7167b50a3ad3ae1ae30799))
* bump ratatui 0.30.0 ([781e156](https://github.com/tekumara/pui/commit/781e156d470b6c46f74293d11beeccef44d8579a))
* cargo format ([9a22165](https://github.com/tekumara/pui/commit/9a22165b375dd6877b77eccd0c41b466857db7ed))


### Builds

* add .release-please-manifest.json ([ec9391f](https://github.com/tekumara/pui/commit/ec9391f217739ebefcf5b40a96f4474affe3ec70))
* add ci, release, release please and dependabot ([0fdad3c](https://github.com/tekumara/pui/commit/0fdad3c4f3aa7ba0a1585b05a867523e739aa178))
* add release-please-config.json ([d1dc800](https://github.com/tekumara/pui/commit/d1dc800a4dae2f4edfa6aace6a1626bb5ead9751))
