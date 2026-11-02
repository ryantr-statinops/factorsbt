# Stage 01 — Hợp đồng giữa các thành phần

**Status:** Proposed

**Phase:** A — Chốt thiết kế MVP

## Mục tiêu

Làm rõ các trao đổi cần cho MVP để SDK, backend, adapters và engine được triển khai tương thích mà vẫn giữ đúng ranh giới trách nhiệm.

## Phạm vi và đầu ra

- Đặc tả thao tác gửi run, kiểm tra tiến trình, báo lỗi và truy xuất kết quả cho lát cắt đã chọn.
- Đặc tả dữ liệu chuẩn hóa, snapshot, model request/response và giao tiếp nội bộ orchestrator/engine.
- Quy định liên kết run, scenario, step, snapshot, decision group, order và fill; lưu provenance của cấu hình, dữ liệu và phản hồi.
- Ghi tác động tới caller, adapter và kết quả lưu trữ khi thay đổi hợp đồng; đánh dấu ví dụ là conceptual cho tới khi chạy được.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md): phạm vi, tài chính, thời gian và đầu ra mô hình đã được chốt cho MVP.
- HTTP/JSON là hướng Accepted; không suy ra rằng giao tiếp nội bộ Rust phải là HTTP.

## Quyết định cần chốt — Open

- Endpoint, schema, validation, timestamp encoding, đơn vị, lỗi và operational lifecycle của run.
- Submission identity/deduplication, tương quan phản hồi, retry ownership và trạng thái khi mất phản hồi HTTP.
- Đồng bộ/bất đồng bộ, polling/streaming, cancellation có nằm trong MVP hay được để sau.
- Payload inline hay snapshot reference; quyền truy cập, lifetime, bảo vệ secret và cách thông báo bằng chứng thiếu.
- Kiểu nội bộ engine, ranh giới cập nhật trạng thái, định dạng kết quả và chính sách version/compatibility cho phạm vi cần dùng.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Đặc tả giao tiếp SDK/backend và lifecycle, lỗi, gửi lại | `docs(contracts): specify MVP run operations` |
| 2 | Đặc tả snapshot, data/model adapter và ranh giới engine | `docs(contracts): specify MVP simulation exchanges` |
| 3 | Định nghĩa evidence, versioning cần thiết và ví dụ đối chiếu hai phía | `docs(contracts): define MVP evidence and compatibility` |

## Kiểm chứng dự kiến

- Đối chiếu producer/consumer trên các mẫu hợp lệ, không hợp lệ và phản hồi sai step/snapshot.
- Mô tả rõ trường hợp mất submission acknowledgment, phản hồi trễ/trùng và thay đổi schema của kết quả.
- Kiểm tra hợp đồng không biến timeout vận hành thành hành động giao dịch hoặc hứa exactly-once khi chưa có bằng chứng.

## Điều kiện hoàn thành

- Hai phía của mỗi ranh giới có cùng định nghĩa I/O, định danh, lỗi và retry ownership cho MVP.
- Có ví dụ làm đầu vào kiểm chứng triển khai; phần chưa chọn được giữ Open, ngoài phạm vi cần thực thi.
- Đặc tả chưa được mô tả là API ổn định hay Implemented chỉ vì đã viết xong.

## Tài liệu liên quan

- [Service contracts](../../docs/interfaces/service-contracts.md)
- [Architecture overview](../../docs/architecture/overview.md)
