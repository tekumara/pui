# Changelog

## [0.1.1](https://github.com/tekumara/pui/compare/v0.1.0...v0.1.1) (2026-02-17)


### Features

* add Ctrl+A select all shortcut ([2d2f664](https://github.com/tekumara/pui/commit/2d2f66484fbd1b182ecf5abfa3f5897a70b1a806))
* add end column ([66ba310](https://github.com/tekumara/pui/commit/66ba310a8ef78e7fed7cd4398412e431a15cdf2e))
* add less-style shortcuts to log viewer ([1f67523](https://github.com/tekumara/pui/commit/1f6752344fcfacffbff841dd85de2b2211db97bf))
* add log viewer ([9b0c2d0](https://github.com/tekumara/pui/commit/9b0c2d0d37d61792a9718fc2f700e14b93473439))
* add row filtering functionality ([c36406f](https://github.com/tekumara/pui/commit/c36406f111ebfd59abfbd2191526c68ddde143c1))
* add scrollbar ([fc642cd](https://github.com/tekumara/pui/commit/fc642cd2bc355b1ca09d3ec70c80e92dc298d5dd))
* add support for PageUp and PageDown ([8d43246](https://github.com/tekumara/pui/commit/8d432462b15f0a75d6200b977db075b8f817dc30))
* add table with columns ([5681cd0](https://github.com/tekumara/pui/commit/5681cd0a2cc353eeaa2f41de3049f1511e22c715))
* custom commands ([7fc8582](https://github.com/tekumara/pui/commit/7fc8582a0e596b5a07ad2a1530cba48f64a7b524))
* initial status UI ([cdc63fb](https://github.com/tekumara/pui/commit/cdc63fb456a4b558dcc9277ff3aed214de81b946))
* move to next line on space, wrapping around ([e3db054](https://github.com/tekumara/pui/commit/e3db0540f771b5526f3499c93d6cc80adce15979))
* multi-select ([78c808a](https://github.com/tekumara/pui/commit/78c808ada5971b3289f0e4ae30fe9e2e39ac23d5))
* scroll to end when opening log viewer ([ea61b63](https://github.com/tekumara/pui/commit/ea61b63dd0ea82d8e4f6a44c76ec6d115a80c771))
* show details as popup ([9c13b5d](https://github.com/tekumara/pui/commit/9c13b5da09ba820e0bde9aaed8e086b14dbe1a10))
* show failure exit code in status column ([74a6a49](https://github.com/tekumara/pui/commit/74a6a49b9ca9244876902da2c4474f46aa5d1b78))
* sort by columns ([34be3fc](https://github.com/tekumara/pui/commit/34be3fc6305a00abf8fb5feb9651900b23c8573a))
* start stashed tasks ([aa4182a](https://github.com/tekumara/pui/commit/aa4182a98693f08301bd0e1adef9819694ffa037))
* use esc to exit log viewer ([a97f31e](https://github.com/tekumara/pui/commit/a97f31e113f7e53c11979db890ca015a83dd69dd))
* use q to exit log viewer ([44cc687](https://github.com/tekumara/pui/commit/44cc687e23d9d10ad0c8adfb99c28d633a6dd205))
* wrap around task table on up/down ([1619706](https://github.com/tekumara/pui/commit/161970639db78633c815f773a618af6b3c3dbcd8))
* wrap long lines in details view ([c29c34f](https://github.com/tekumara/pui/commit/c29c34fd33f08ef1a147be7106f4e9293e829a0d))


### Bug Fixes

* add select and logs keyboard shortcuts to the header ([b0cf54f](https://github.com/tekumara/pui/commit/b0cf54f210cbb2437508ff3fd54ae1b8d7d48825))
* after removing last task move to previous row ([eea1bab](https://github.com/tekumara/pui/commit/eea1bab40443c8dbe993b9d113e72b63973af556))
* allow tests to run without requiring stdin ([d79a9d3](https://github.com/tekumara/pui/commit/d79a9d3b5b74a7b80017f416ceb74a87f2210e14))
* clippy warnings ([b514314](https://github.com/tekumara/pui/commit/b5143148e669d260af6b47df48e9a2879efbfb97))
* display status name instead of debug format ([8406991](https://github.com/tekumara/pui/commit/840699130c8a537d9ea6372f3b00d3a1f2db8944))
* don't hang when viewing logs for stashed task ([c3e5d5b](https://github.com/tekumara/pui/commit/c3e5d5b3718ccfe75d694c09d94297fc5d2d7e37))
* don't loop back to top at end of task table ([1674998](https://github.com/tekumara/pui/commit/16749987e61b7abb35f5c9508af6caf7d9bcc029))
* Esc preserves selection when exiting filter ([f581db5](https://github.com/tekumara/pui/commit/f581db5f910043585dd15e3266b8cb91c1417d20))
* give Path column more space in task table ([f3e054f](https://github.com/tekumara/pui/commit/f3e054f0206dcb92c0c7a137e7336971c5860ea8))
* log viewer correctly scrolls to the end ([afa40ed](https://github.com/tekumara/pui/commit/afa40ed255476eb4630395412cfbce6695cf90b0))
* log viewer overscrolling and producing blank lines ([bf12719](https://github.com/tekumara/pui/commit/bf1271927dac6e285b5f41bf4c0d02901633558a))
* make custom command tests cross-platform for Windows ([a5751c6](https://github.com/tekumara/pui/commit/a5751c6c871a7398c52ba58b4bfc03ccd41064d9))
* move cursor to top-left before running custom command ([f763ee7](https://github.com/tekumara/pui/commit/f763ee737dadcaba6b36de910c603f0456a6cb52))
* move selection to previous task on remove ([10a2a18](https://github.com/tekumara/pui/commit/10a2a18d70576d55206139059ef875b209cf76e3))
* PgDn/PgDn no longer scrolls back after returning from custom command ([043420a](https://github.com/tekumara/pui/commit/043420a713c661e58b898a7a470f70bd7c91117a))
* refresh state immediately after task modification ([d7150e2](https://github.com/tekumara/pui/commit/d7150e24524423373495b0af5175b46cb3c50083))
* removing running or paused task will do nothing ([f7eed4e](https://github.com/tekumara/pui/commit/f7eed4eb635e44cc4c9966d40e805feab836175b))
* render help modal scrollbar inside the border ([8b2845f](https://github.com/tekumara/pui/commit/8b2845f898768e2394dc0b5748bf3756a9a3727a))
* render tabs and wrap lines in log viewer ([b5337da](https://github.com/tekumara/pui/commit/b5337da7055f5e122b54acb8db59a60f296a9326))
* scroll bar thumb in help modal now scrolls to the bottom ([ca35e83](https://github.com/tekumara/pui/commit/ca35e834bec029d51a90f4d4837c7c6f4dc5d141))
* start will restart failed/succeeded task ([49b2d48](https://github.com/tekumara/pui/commit/49b2d485efce62f48abed6e2f94b670992bb3e11))
* use %APPDATA%\pui\config.toml on Windows ([7ea74a4](https://github.com/tekumara/pui/commit/7ea74a453ac20a6649896e09f49419d2da626858))
* **windows:** connect to pueued in platform agnostic way ([378fd53](https://github.com/tekumara/pui/commit/378fd53b32414f07fcc4e01f5834d8b24e422a7f))


### Chores

* bump crossterm 0.29.0 ([86a43e3](https://github.com/tekumara/pui/commit/86a43e32353e919fbedd930a48ad73a59f18f380))
* bump ratatui 0.30.0 ([8fa6baa](https://github.com/tekumara/pui/commit/8fa6baa68025b06ec126355e999126eb0d1022b9))
* cargo format ([06ec1e4](https://github.com/tekumara/pui/commit/06ec1e440de90329b7a4a12b5427fe60101aeb87))


### Styles

* abbreviate path column ([c31c2ee](https://github.com/tekumara/pui/commit/c31c2ee7eea8f798d7fdd0cd0a1fa4be61166f6a))
* balance path and command column padding ([4ae5590](https://github.com/tekumara/pui/commit/4ae5590a71b0d68aa0fdf6a058299233c523cc53))
* reorder and simplify keybinding hints in header ([a47249b](https://github.com/tekumara/pui/commit/a47249bdc86111c5e8d772b20b0f189e3e8cf847))
* show connection errors in footer, action errors in modal ([18a28e6](https://github.com/tekumara/pui/commit/18a28e645f15fd72e10af8ce5e3db01b0b45dac2))


### Tests

* add integration test for terminal restoration after custom commands ([9d8c495](https://github.com/tekumara/pui/commit/9d8c4951344b445d578a48b21a80996e11abf7bc))
* add snapshot testing of ui ([8311572](https://github.com/tekumara/pui/commit/8311572bb3154ab612cc753d9288d67accf71fe1))


### Builds

* add .release-please-manifest.json ([28f822e](https://github.com/tekumara/pui/commit/28f822e38102c813b07457b669e9283ae60a8e79))
* add cargo config.toml ([0bec49e](https://github.com/tekumara/pui/commit/0bec49e5bcaed37469033f9ab663874c4558a624))
* add ci, release, release please and dependabot ([f94724a](https://github.com/tekumara/pui/commit/f94724a2c9617bc3ada19c6412cfd83f2dd08837))
* add release-please-config.json ([da5fe4a](https://github.com/tekumara/pui/commit/da5fe4a51e3212caa7abeb52e7a25eeed3e7667d))
* **release-please:** add style + test headings ([53e22ca](https://github.com/tekumara/pui/commit/53e22ca9a36cec386070e2da905fb564e4cea3cd))
* **release:** exclude component name from release tag ([bbed4cc](https://github.com/tekumara/pui/commit/bbed4cc0c600796e249047f8228122061e3a701e))
* **release:** install musl-tools ([6181b88](https://github.com/tekumara/pui/commit/6181b8883a5705ee42c22d7f9f04d18021c4bb68))
* **release:** remove the gnu builds ([3356d05](https://github.com/tekumara/pui/commit/3356d05136f66b64870f82163e186ea05939c118))
