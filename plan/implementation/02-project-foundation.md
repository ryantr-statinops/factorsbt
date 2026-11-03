# Stage 02 — Nền tảng dự án và công cụ phát triển

**Status:** Implemented — foundation and health flow verified on the Windows MSVC host.

**Phase:** A — Chốt thiết kế MVP

## Mục tiêu

Tạo nền tảng chạy và phát triển tối thiểu cho SDK Python và backend/engine Rust, không đưa thêm hạ tầng trước nhu cầu của MVP.

## Phạm vi và đầu ra

- Thiết lập Cargo workspace `rust/` với contracts, engine, backend và Python package `python-sdk/src/factorsbt/`.
- Dùng Python standard library cho SDK; Rust dùng Axum/Tokio/Serde cho API health và JSON.
- Ghi bằng chứng toolchain preflight; không tự cài MSVC/Windows SDK và không báo compile/run thành công khi thiếu linker.
- Thiết lập health check, shared fixtures và Python-to-Rust smoke test; không thêm simulation, database, queue hay CI.
- Chỉ ghi lệnh setup/format/lint/test sau khi xác minh thực tế; chưa công bố supported-version range khi chưa có bằng chứng.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md): môi trường chạy, phạm vi và ranh giới kiểm chứng.
- [Stage 01](01-contracts.md): hợp đồng ban đầu để cấu trúc module phục vụ đúng nhu cầu.
- Lệnh tooling và kết quả xác minh hiện tại được ghi tại [Phase A local development](../../docs/development/phase-a-local.md).

## Quyết định đã chốt cho lát cắt Phase A

- Rust workspace gồm contracts/engine/backend; API local dùng Axum/Tokio/Serde.
- Python SDK dùng standard library, synchronous HTTP client và `unittest`; không embed Rust.
- Không có database, queue, auth, restart recovery, remote deployment hoặc CI trong Phase A.
- Engine crate chỉ là skeleton, chưa chứa backtest/pricer.

## Prerequisite môi trường hiện tại

Toolchain đã được cấp trên host này: Visual Studio 2022 Build Tools 17.14.44, MSVC 14.44.35207 và Windows SDK 10.0.26100.0; Rust/Cargo 1.99.1 target `x86_64-pc-windows-msvc`, Python 3.13.13. Kết quả cài đặt và kiểm chứng được ghi trong [ADR 0002](../../docs/decisions/0002-phase-a-tooling.md#follow-up-verification-2026-11-03).

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | ADR công cụ, boundaries và ghi nhận prerequisite thật của môi trường | `docs(architecture): record Phase A tooling choices` |
| 2 | Cargo workspace và engine crate tối thiểu | `chore(rust): initialize backend engine workspace` |
| 3 | Contracts crate với ID, timestamp và decimal wire primitives | `feat(contracts): add shared Rust wire primitives` |
| 4 | Rust instrument types và model exchanges | `feat(contracts): add instrument and decision wire types` |
| 5 | Rust run operations, outcomes và evidence types | `feat(contracts): add run and result wire types` |
| 6 | Python package, dataclasses/decimal parsing và checks liên quan | `chore(python): initialize standard-library SDK package` |
| 7 | Backend loopback health endpoint và response check | `feat(backend): add local health endpoint` |
| 8 | SDK transport, `Client.health()` và lỗi kết nối/timeout | `feat(sdk): add standard-library health client` |
| 9 | Kiểm tra contract examples dùng chung qua schema, Rust và Python | `test(contracts): verify cross-language contract examples` |
| 10 | Smoke test khởi chạy Rust backend, gọi SDK và cleanup | `test(integration): verify Python-to-Rust health flow` |
| 11 | Hướng dẫn prerequisite và lệnh đã thực sự xác minh | `docs(dev): document verified Phase A commands` |
| 12 | Cập nhật trạng thái, bằng chứng và graph Phase A sau khi qua cổng | `docs(plan): close Phase A verification` |

## Kiểm chứng dự kiến

- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`, `cargo clippy`, `cargo check`, `cargo test`, Python `unittest`, schema/contract checks và integration smoke đều đã chạy thành công.
- Xác minh SDK không nhúng Rust engine; engine không phụ thuộc HTTP retry policy.
- Ghi lệnh và toolchain thực tế trong [Phase A local development](../../docs/development/phase-a-local.md); CI không được thiết lập trong stage này.

## Điều kiện hoàn thành

- Python/Rust workspace build/check/test; Clippy và formatter pass.
- Shared contract examples pass; Python-to-Rust smoke gọi được health endpoint và cleanup hoàn tất.
- Development workflow ghi đúng phiên bản toolchain và lệnh đã xác minh. Không bao gồm simulation, pricing hoặc API chạy backtest.

## Tài liệu liên quan

- [Development workflow](../../docs/development/workflow.md)
- [Architecture overview](../../docs/architecture/overview.md)
- [ADR 0001](../../docs/decisions/0001-python-sdk-rust-backend.md)
