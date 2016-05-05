# Change Log
All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [0.1.1] - 2016-01-31
### Added
  - Add parser based on html5ever
  - Add namespace parsing

## [0.1.2] - 2016-04-26
### Added
  - Quiescent state for interrupting parser from @ConnorGBrewster

### Fixed
  - Bug in namespace parsing of end tag from @bpowers
  - Removed mention of `one_input` from README.md from @Ygg01

## [0.1.3] - 2016-05-04
### Fixed
  - `complete_script` popped the open script tag instead of getting the current node
