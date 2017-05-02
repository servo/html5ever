# Change Log
All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).

## [0.6.0] - 2017-04-17
### Changed
  - Replace html5ever_macros with markup5ever
  - Move common interface between into markup5ever

## [0.5.0] - 2017-04-07
### Changed
  - Breaking string-cache update

## [0.4.1] - 2017-02-17
### Changed
  - Changed dependency constraints to be more precise

## [0.4.0] - 2017-02-17
### Changed
  - Updated html5ever-atoms to 0.2

## [0.3.2] - 2017-01-11
### Changed
  - Replaced `println!` with `debug!`in tokenizer @Freyskeyd

## [0.3.1] - 2017-01-06
### Added
  - Mechanism to signal sink that a node was popped @nox

## [0.3.0] - 2016-12-04
### Added
  - Support for XML encoding @Ygg01
  - Serializer for XML @Ygg01
  - Test for serializing namespace @Ygg01

### Changed
  - Removed `tokenize_to` method @Ygg01
  - Moved parse into a separate driver module @Ygg01
  - Moved `atoms!` macro from src/tree_builder/mod.rs into src/lib.rs @Ygg01
  - Made NamespaceStack publicly visible but hidden. @Ygg01
  - Changed serialization rules, to serialize namespace @Ygg01
  - Changed rules for comment parsing. @Ygg01

## [0.2.0] - 2016-11-02
### Added
  - Add `LocalName`, `Prefix`, `Namespace` types. @SimonSapin
  - Added `html5ever_macros` instead of `string_cache`. @SimonSapin

### Changed
  - Changes API names: @SimonSapin
    - `Namespace` -> `NamespaceMap`
    - `NamespaceStack`-> `NamespaceMapStack`


### Removed
  - Removes `string_cache` in favor of `html5ever_macros`. @SimonSapin

## [0.1.3] - 2016-05-04
### Fixed
  - `complete_script` popped the open script tag instead of getting the current node

## [0.1.2] - 2016-04-26
### Added
  - Quiescent state for interrupting parser from @ConnorGBrewster

### Fixed
  - Bug in namespace parsing of end tag from @bpowers
  - Removed mention of `one_input` from README.md from @Ygg01 

## [0.1.1] - 2016-01-31
### Added
  - Add parser based on html5ever
  - Add namespace parsing