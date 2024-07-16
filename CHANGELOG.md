# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] - 202?-??-??
### Added
- end of beta

## [0.2.3] - 2024-07-16
### Fixed
- Dockerfile alpine based works now
### Changed
- change gallery ratio to fit better with comics cover
- add blur effect on comics cover (thanks @lmartellotto)
- increase cover sizes, more use of network and disk :( (about 16 ko per cover)

## [0.2.1] - 2024-07-12
### Changed
- move assets (src, images, font) to project root dir
- upgrade to compress-tools 0.15 (should fix panic issue [#126](https://github.com/OSSystems/compress-tools-rs/pull/126) of the lib)

## [0.2.0] - 2024-06-26
### Added
- new CSS (thanks @lmartellotto) with Sass
- nix flakes
- enable http2 (useless)
### Changed
- migration to axum-login 0.10 and axum 0.7
- update README and screenshots
- update dependencies
- temporary fix compress-tools panic
- speed up build time (10s), no need of debuginfo
### Fixed
- Dockerfile images path

## [0.1.3] - 2023-12-05
### Changed
- generic page for authent error
- store hashed password in database (#4)
- no more config file
### Fixed
- sub-directory count (#8)
- return link (#15)

## [0.1.2] - 2023-08-31
### Changed
- upgrade dependencies (sqlx, axum-login), fixing some CVEs

## [0.1.1] - 2023-07-24
### Added
- first "usable" release with base library scanning, comic / ebook reader, user management...

## [0.1.0] - 2023-07-24
### Added
- init
