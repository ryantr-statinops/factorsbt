# Stage 06 — Backend API và quản lý run

**Status:** Proposed

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Cho client gửi thí nghiệm, theo dõi trạng thái vận hành và lấy kết quả qua Rust backend HTTP/JSON theo hợp đồng MVP đã chốt.

## Phạm vi và đầu ra

- Tiếp nhận và validate cấu hình, cấp run identity, điều phối thực thi và báo tiến trình/lỗi.
- Cung cấp thao tác tra cứu kết quả và evidence được phép truy cập; phân biệt submission, waiting và execution failure.
- Áp dụng submission identity/deduplication và retry ownership đã chọn để xử lý mất HTTP response.
- Tách operational lifecycle của run khỏi simulation clock và trạng thái tài chính.
- Ghi cấu hình đã dùng, lỗi có ngữ cảnh và giới hạn vận hành; bảo vệ secrets trong log và response.

## Phụ thuộc

- [Stage 01](01-contracts.md), [Stage 02](02-project-foundation.md): HTTP contract, framework và nền tảng vận hành.
- [Stage 05](05-model-orchestration.md): đường chạy điều phối và evidence, bao gồm data/engine phía dưới.

## Quyết định cần chốt — Open

- Endpoint/schema, run lifecycle, polling/streaming và cancellation nếu được đưa vào MVP.
- Lưu trữ, retention, worker/concurrency, giới hạn request và hành vi khi tiến trình dừng; không mặc định queue hoặc cơ chế durable.
- Authentication/authorization và môi trường truy cập cho phạm vi triển khai thực tế; không tự chọn hosted service hay tenancy.
- Phạm vi deduplication, thời hạn identity, lỗi xung đột và khả năng tra cứu khi acknowledgment bị mất.
- Access control cho artifact/snapshot reference, pagination hoặc giới hạn payload cần cho MVP.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Triển khai submission/validation và identity chống hiệu ứng trùng | `feat(api): accept validated backtest runs with submission identity` |
| 2 | Nối orchestrator và công bố lifecycle/progress/failure | `feat(api): expose run progress and execution failures` |
| 3 | Truy xuất kết quả/evidence với giới hạn truy cập đã chọn | `feat(api): expose traceable backtest results` |

## Kiểm chứng dự kiến

- Cấu hình hợp lệ và không hợp lệ cho acknowledgment/lỗi đúng hợp đồng; không bắt đầu run bị từ chối.
- Submission được nhận nhưng HTTP response bị mất có thể được xác định lại theo cơ chế đã chọn, không coi client timeout là chắc chắn thất bại.
- Retry/trùng submission không nhân đôi hiệu ứng trong phạm vi bảo đảm được ghi rõ.
- Adapter failure được thể hiện ở run đúng ngữ cảnh; xem tiến trình không làm thay đổi đồng hồ mô phỏng.
- Kết quả nối được đến evidence; kiểm tra truy cập và secret redaction theo mô hình truy cập đã chọn.

## Điều kiện hoàn thành

- Các thao tác run/results thực sự chạy được và có kiểm chứng qua HTTP với controlled adapters.
- Contract phản ánh hành vi triển khai; không công bố stable API hoặc exactly-once vượt bằng chứng.
- Giới hạn persistence, restart, concurrency và truy cập được ghi rõ thay vì hứa khả năng chưa triển khai.

## Tài liệu liên quan

- [Service contracts](../../docs/interfaces/service-contracts.md)
- [Backend API role](../../.agents/roles/backend-api.md)
