# Stage 05 — Model adapter và điều phối mô phỏng

**Status:** Proposed

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

## Quyết định cần chốt — Open

- Model service đầu tiên, output contract và cách xử lý đầu ra không hợp lệ/không đủ.
- Timeout/retry limits, run failure behavior và quyền sở hữu thao tác gửi lại.
- Stateful model có được dùng không; nếu có, cơ chế session/reset/isolation phải được định nghĩa và kiểm chứng.
- Service latency bị bỏ qua bởi simulation clock, được mô hình hóa hay thành scenario riêng; không tự dùng thời gian chờ thực làm trading delay.
- Cách giữ phản hồi, model version và tham chiếu snapshot mà không lộ secret hoặc dữ liệu hạn chế.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Xây model adapter, validation và controlled service fixture | `feat(models): add correlated MVP inference adapter` |
| 2 | Nối snapshot, model và engine thành decision cycle | `feat(orchestrator): coordinate simulation decision cycles` |
| 3 | Xử lý failure/duplicate và ghi phản hồi, trạng thái bước | `feat(orchestrator): handle inference failures and duplicate responses` |

## Kiểm chứng dự kiến

- Một phản hồi chứa nhiều quyết định được xét thành nhóm trên cùng snapshot.
- Request timeout, malformed output, phản hồi sai scenario/step/snapshot và phản hồi trễ có kết quả đúng chính sách.
- Gửi lại/nhận trùng không tạo hai lần chuyển trạng thái cho một nhóm đã áp dụng trong phạm vi bảo đảm đã chọn.
- Thay thời gian chờ dịch vụ không tự đổi simulation time khi policy không quy định ánh xạ.
- Model state và dữ liệu nhạy cảm được xử lý theo ranh giới đã chấp nhận, không suy ra deterministic inference từ endpoint hoặc seed.

## Điều kiện hoàn thành

- Chạy được một chu kỳ đầy đủ với model fixture và đường dịch vụ đã chọn trong phạm vi truy cập có thể xác minh.
- Failure và duplicate có evidence; phản hồi không thể cập nhật sai scenario/step trong các trường hợp đã kiểm tra.
- Engine vẫn sở hữu vốn/thực thi, orchestrator sở hữu điều phối và I/O.

## Tài liệu liên quan

- [Architecture overview](../../docs/architecture/overview.md)
- [Service contracts](../../docs/interfaces/service-contracts.md)
