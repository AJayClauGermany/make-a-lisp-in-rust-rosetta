# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Cargo workspace with `mal-traits` (component traits, step and component
  vocabulary, `Implementation` metadata, REPL driver) and `mal` (the REPL
  binary with a dependency-free step0 baseline and `--list`).
- Documentation site scaffold: mdBook under `docs/` and oranda configuration.
- CI: format, clippy, test, docs, `cargo deny`, and the oranda site build.
