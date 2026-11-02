# Stage 02 — Nền tảng dự án và công cụ phát triển

**Status:** Proposed

**Phase:** A — Chốt thiết kế MVP

## Mục tiêu

Tạo nền tảng chạy và phát triển tối thiểu cho SDK Python và backend/engine Rust, không đưa thêm hạ tầng trước nhu cầu của MVP.

## Phạm vi và đầu ra

- Thiết lập manifests và cấu trúc mã nguồn tách SDK, orchestration, adapters và engine theo trách nhiệm.
- Chọn dependency, runtime/toolchain và công cụ kiểm tra cần thiết sau khi chốt yêu cầu; ghi rõ phiên bản hỗ trợ có bằng chứng.
- Thiết lập đường chạy tối thiểu, fixture dùng chung và cách cấu hình cục bộ không chứa secret thật.
- Cập nhật development workflow với prerequisite và lệnh đã chạy thành công; CI chỉ được thêm khi phạm vi triển khai tương lai cho phép.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md): môi trường chạy, phạm vi và ranh giới kiểm chứng.
- [Stage 01](01-contracts.md): hợp đồng ban đầu để cấu trúc module phục vụ đúng nhu cầu.
- Không có build/test/lint command được thiết lập ở thời điểm viết kế hoạch này.

## Quyết định cần chốt — Open

- Rust framework, Python HTTP client, package layout, công cụ quản lý dependency và toolchain.
- Chạy nội bộ hay triển khai dịch vụ, storage tối thiểu và yêu cầu truy cập; queue không được mặc định là cần thiết.
- Công cụ định dạng, phân tích, test; phạm vi CI nếu được lựa chọn và khả năng chạy trong môi trường hỗ trợ.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Ghi quyết định công cụ và cấu trúc triển khai; ADR khi đáng kể | `docs(architecture): record MVP tooling choices` |
| 2 | Tạo manifests và skeleton Python/Rust tối thiểu | `chore(project): initialize Python and Rust foundations` |
| 3 | Thiết lập kiểm tra đã chọn, fixture và hướng dẫn chạy đã xác minh | `chore(dev): establish verified development checks` |

## Kiểm chứng dự kiến

- Cài đặt và chạy đường tối thiểu từ môi trường sạch thuộc phạm vi hỗ trợ đã chọn.
- Xác minh SDK không cần nhúng Rust engine; module engine không phụ thuộc chính sách HTTP retry.
- Chạy các kiểm tra thật sự tồn tại sau khi thiết lập; ghi lệnh, kết quả và giới hạn môi trường.

## Điều kiện hoàn thành

- Python/Rust có cấu trúc và công cụ chạy được, ranh giới thành phần rõ ràng.
- Development workflow phản ánh đúng lệnh đã kiểm chứng, không chứa lệnh suy đoán hoặc credentials.
- Các stage sau có thể thêm hành vi mà không phải chọn lại nền tảng thiết yếu.

## Tài liệu liên quan

- [Development workflow](../../docs/development/workflow.md)
- [Architecture overview](../../docs/architecture/overview.md)
- [ADR 0001](../../docs/decisions/0001-python-sdk-rust-backend.md)
