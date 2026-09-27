# Changelog

## [0.4.0] 2024-08-06

- breaking: change primary key type from `i32` to `i64`.
- breaking: change the `ptype` field type from `varchar(12)` to `varchar(18)`.
- chore: simplify the code.
- dep: update `sea-orm` to `1`.

## [0.3.4] 2024-05-02

- fix: consistent with the behavior of functions in `sqlx-adapter` crate.

## [0.3.3] 2024-04-24

- Fix issue caused by `casbin` breaking semantic version specification.
- chore: make clippy happy.

## [0.3.2] 2023-07-29

- Update `sea-orm` to `0.12`.

## [0.3.1] 2023-03-27

- Downgrade `sea-orm` to `0.11.2`.

## [0.3.0] 2023-03-26

- Update `sea-orm` to `0.12.0`.
- Fix: pg auto_increment. ([#1](https://github.com/ZihanType/sea-orm-adapter/pull/1))
- Include `README.md` as documentation.
- Fix: expose mod `entity` instead of items in it.
- Fix: re-generate `entity` mod when `sea-orm` version changes.

## [0.2.0] 2023-02-08

- Update `sea-orm` to `0.11.0`.

## [0.1.0] 2023-01-1

- Add `SeaOrmAdapter`.
