# Stage 05 — Model adapter và điều phối mô phỏng

**Status:** Implemented

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Nối snapshot, lời gọi mô hình và engine thành chu kỳ mô phỏng có tương quan rõ ràng; lỗi dịch vụ không được tự biến thành quyết định giao dịch.

## Phạm vi và đầu ra

- Tích hợp một model service đã chọn qua adapter, cùng dịch vụ giả lập có phản hồi controlled.
- Điều phối eligible data → data/portfolio snapshot → inference → validation → engine → evidence.
- Gắn request/response với run, scenario, step và snapshot; giữ model identity/version khi có.
- Xử lý phản hồi sai, trễ hoặc trùng theo hợp đồng; giới hạn timeout/retry và không áp dụng decision group hai lần.
- Ghi phản hồi dùng trong run và trạng thái lỗi; không tạo quyết định thay thế không được chấp nhận.

## Phụ thuộc

- [Stage 01](01-contracts.md), [Stage 02](02-project-foundation.md): hợp đồng model và công cụ.
- [Stage 03](03-data-snapshots.md), [Stage 04](04-simulation-engine.md): snapshot khả dụng và engine chuyển trạng thái.
- Decision trigger và chính sách failure cần được chốt từ Stage 00 hoặc được bổ sung trước task liên quan.

## Quyết định đã Accepted cho Phase B

- MVP gọi một HTTP model fixture stateless ở loopback; output phải khớp chính xác một trong ba response modes của contract v1.
- Áp dụng `timeout_ms` theo run, không tự retry và không theo redirect. Timeout, response malformed/invalid hoặc correlation mismatch làm run thất bại.
- Simulation time không tiến theo thời gian chờ dịch vụ. Mỗi inference gắn với run, scenario, step, snapshot và model identity; stale hoặc duplicate response không được áp dụng.
- Hai run có thể inference đồng thời nhưng sở hữu state và evidence writer riêng. Fixture/model output không bị chia sẻ như mutable state.
- Lưu request/response đã dùng cùng provenance trong evidence local; không ghi credential hoặc payload nhạy cảm vào log/error.

## Task → Commit

| # | Kết quả đã commit | Commit |
| --- | --- | --- |
| 31 | Controlled HTTP model fixture | `8a7b948 test(models): add controlled HTTP inference service` |
| 32 | Async loopback HTTP client, timeout, không retry/redirect | `0e9acc2 feat(models): call the local correlated inference service` |
| 33 | Validation ba output modes và correlation | `aaf5c3a feat(models): validate correlated decision outputs` |
| 34 | Pending inference identity, stale/duplicate guard | `5836942 feat(orchestrator): guard inference response application` |
| 35 | Data → snapshot → model → engine decision cycle | `259d2c8 feat(orchestrator): execute complete simulation cycles` |
| 36 | Evidence writer và digest/size theo bytes lưu | `b72ef92 feat(evidence): persist traceable run artifacts` |
| 37 | Failure evidence và last valid state | `9fb141b feat(orchestrator): preserve failed run evidence` |
| 38 | Hai run đồng thời, response isolation và evidence riêng | `806e2ba test(orchestrator): verify concurrent run isolation` |

## Kiểm chứng đã chạy

- `cargo test -p factorsbt-backend --offline`: 50 tests pass sau khi hoàn thành Stage 06; suite bao gồm adapter, model validation, inference guard, orchestrator và concurrency.
- Các kiểm tra trực tiếp bao phủ timeout, HTTP failure, response malformed, redirect không được theo, endpoint non-loopback, correlation mismatch/stale response và tách evidence cho hai run.
- `cargo clippy -p factorsbt-backend --all-targets --offline` và `cargo fmt --all -- --check` pass ở lần xác minh Stage 06.
- Đường chạy đầy đủ qua HTTP model fixture, engine, result và evidence được xác minh thêm tại [Stage 06](06-backend-api.md).

## Điều kiện hoàn thành

- Chạy được decision cycle với model fixture, engine và evidence; failure giữ ngữ cảnh inference/last valid state khi có.
- Hai run đồng thời giữ correlation, portfolio và evidence riêng; response đảo thứ tự không bị gán chéo.
- Engine sở hữu vốn/thực thi, orchestrator sở hữu điều phối và I/O. Việc triển khai Stage 05 không khẳng định inference có tính deterministic.

## Tài liệu liên quan

- [Architecture overview](../../docs/architecture/overview.md)
- [Service contracts](../../docs/interfaces/service-contracts.md)
