# Stage 02 — Nền tảng dự án và công cụ phát triển

**Status:** Proposed — implementation incomplete; local native build prerequisite is missing.

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

Preflight ghi nhận Python 3.13.13, Rust/Cargo 1.99.1 và target `x86_64-pc-windows-msvc`. Không tìm thấy MSVC C++ linker/toolset hoặc Windows SDK. Python contract examples đã được kiểm tra bằng JSON Schema Draft 2020-12 trong virtualenv tạm với validator test-only. Cần môi trường MSVC/Windows SDK tương thích để build backend, chạy Rust tests và integration smoke. Không tự cài công cụ hệ thống trong stage này.

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
| 12 | Ghi nhận bằng chứng và graph Phase A; chỉ đánh dấu hoàn tất sau khi qua mọi cổng | `docs(plan): record Phase A verification status` |

## Kiểm chứng dự kiến

- Chạy `cargo fmt`, compile/check/test Rust, Python `unittest`, schema/contract checks và integration smoke khi prerequisite đủ.
- Xác minh SDK không nhúng Rust engine; engine không phụ thuộc HTTP retry policy.
- Ghi riêng checks có thể chạy không cần linker với checks yêu cầu binary; nếu prerequisite thiếu thì giữ Stage Proposed và không push/không ghi Implemented.

## Điều kiện hoàn thành

- Python/Rust skeleton được build/check; SDK gọi được health endpoint của Rust backend.
- Shared contract examples qua được kiểm chứng; Python-to-Rust smoke cleanup hoàn tất.
- Development workflow chỉ ghi lệnh và prerequisite đã xác minh. Thiếu linker/SDK đồng nghĩa stage chưa hoàn thành.

## Tài liệu liên quan

- [Development workflow](../../docs/development/workflow.md)
- [Architecture overview](../../docs/architecture/overview.md)
- [ADR 0001](../../docs/decisions/0001-python-sdk-rust-backend.md)
