# Crate Publishing Guide

The following crates are on a synchronised release cycle with version managed through the workspace `Cargo.toml`:

- **match_token**
- **markup5ever**
- **xml5ever**
- **html5ever**

The **markup5ever_rcdom** crate's version is also set to match these crates. But it is set to `publish = false` and isn't
published to crates.io.

The **web_atoms** crate is on a separate cycle as it needs frequent releases but these rarely contain breaking changes.

## Making a release of **web_atoms**:

- Bump the version in `web_atoms/Cargo.toml`
   - If just adding an atom, the patch version should be bumped
   - If upgrading `phf` version the minor version should be bumped as this is breaking
- Update the version **web_atoms** in the workspace `Cargo.toml`'s `[workspace.dependencies]` section to match
- Publish the new version of **web_atoms**
- Optionally: publish a new version of the other crates to match

## Making a release of all other crates

In the workspace `Cargo.toml`:

- Update the `version` key in the `[workspace.package]` section
- Update the versions for `match_token`, `markup5ever`, `xml5ever`, and `html5ever` in the `[workspace.dependencies]` section to match
- Publish all of the crates. The order they are listed in at the top of this file will work.
