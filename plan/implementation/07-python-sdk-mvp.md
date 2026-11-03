# Stage 07 — Python SDK và nghiệm thu MVP xuyên suốt

**Status:** Implemented — local typed Python-to-Rust synthetic MVP verified; no package publication or hosted service.

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Đưa lát cắt MVP tới người dùng Python: cấu hình, gửi, theo dõi và đọc một backtest nhiều tài sản có kết quả giải thích được.

## Phạm vi và đầu ra

- Triển khai SDK thuần Python giao tiếp backend HTTP/JSON; simulation và inference không chuyển vào SDK.
- Cho phép đặt backend address, thiết lập kết nối và cấu hình truy cập theo contract đã chọn.
- Tách submit khỏi wait; biểu diễn validation, transport, external-service và execution failure với identity/ngữ cảnh cần thiết.
- Cung cấp ví dụ chạy thực tế với controlled fixtures và đường tích hợp data/model đã chọn; ghi rõ phần nào cần quyền truy cập bên ngoài.
- Trả kết quả liên kết cấu hình → snapshot → model response → admission → order → fill → portfolio; chỉ chọn metric khi định nghĩa đã được thống nhất.

## Phụ thuộc

- [Stage 01](01-contracts.md), [Stage 02](02-project-foundation.md): client contract và nền tảng Python.
- [Stage 06](06-backend-api.md) và các stage data/engine/orchestration phía dưới: đường chạy backend đầy đủ.
- Tiêu chí MVP và ranh giới kiểm chứng đã được chốt từ [Stage 00](00-mvp-decisions.md).

## Quyết định đã Accepted cho Phase B

- SDK tiếp tục dùng standard library và API đồng bộ; configuration và response được biểu diễn bằng typed builders/dataclasses.
- `submit(config, submission_id=None)` tạo UUID trước khi gửi nếu caller không truyền ID; lỗi acknowledgment không tự gửi POST lại và phải giữ submission ID để tra cứu.
- Cung cấp `get_submission`, `get_run`, `get_results` và `wait`. `wait` poll mỗi 0.25 giây, timeout cục bộ mặc định 60 giây bằng monotonic clock, không hủy run; run failure dùng typed exception.
- Lấy result là thao tác riêng sau khi run thành công. Không thêm metric ngoài những gì result contract đã định nghĩa.
- Phase B chỉ xác minh cách chạy package trong checkout/virtualenv; chưa công bố package hay thêm runtime dependency bên thứ ba.

## Task → Commit đã thực hiện

| Commit | Kết quả |
| --- | --- |
| 49 | SDK wire primitives và validation |
| 50–51 | Typed instrument/run configuration builders và ba decision policies |
| 52–55 | Typed responses, submit, lookup, results, polling và typed failures |
| 56–58 | End-to-end smoke cho ba output modes, failure paths, independent NAV và evidence checks |
| 59 | Hướng dẫn môi trường và lệnh đã xác minh |
| 60 | Cập nhật trạng thái Stage 03–07, phase graph và cổng MVP |

## Kiểm chứng đã chạy

- `cargo fmt --all -- --check`, `cargo check --workspace --all-targets --offline`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, và `cargo test --workspace --offline` pass; Rust workspace có 140 tests.
- Python SDK unittest pass: 40 tests. JSON Schema/shared examples và Rust/Python wire handling cũng được xác minh theo hướng dẫn local.
- End-to-end smoke chạy qua typed SDK, Rust backend, model fixture, data và engine cho `scores`, `targets` và `order_intents` trên danh mục fixture gồm bảy instrument. Script kiểm tra market value/NAV độc lập với Decimal và đối chiếu SHA-256/size của từng artifact trên disk.
- Failure-path smoke xác minh mất acknowledgment và tra cứu theo submission ID, cùng key khác config trả conflict, model HTTP 503 làm run thất bại và `wait` timeout cục bộ không cancel run.
- Lệnh thực thi, phiên bản toolchain, kết quả và giới hạn được ghi trong [hướng dẫn Phase B](../../docs/development/phase-b-local.md); CI chưa được cấu hình.

## Cổng hoàn thành — Đã đạt

- Người dùng có thể chạy backtest synthetic xuyên suốt theo hướng dẫn và truy vết kết quả/evidence.
- Gates Phase B về temporal data, engine rules, orchestration, API lifecycle, typed SDK, controlled outputs và các failure paths có bằng chứng đã nêu ở trên.
- Docs phản ánh phần Implemented và giới hạn local. Phase C, D và E không phải phụ thuộc để đóng MVP.

## Giới hạn đã xác minh

- Đạt MVP synthetic local theo fixture đã chốt; không tích hợp data provider thật hoặc market/model endpoint bên ngoài.
- Metadata và deduplication ở process backend; evidence nằm trên filesystem local. Chưa có database, restart recovery, remote artifact download, cancellation, streaming, authentication, hosted deployment hoặc public API stability.
- Chưa công bố package Python. CI chưa được cấu hình; các lệnh được ghi lại đã chạy thủ công trong môi trường Windows/MSVC.

## Tài liệu liên quan

- [Roadmap — first complete backtest path](../../docs/roadmap.md#stage-1-first-complete-backtest-path)
- [Python SDK role](../../.agents/roles/python-sdk.md)
- [Development workflow](../../docs/development/workflow.md)
