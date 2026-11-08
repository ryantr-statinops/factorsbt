# Stage 01 — Hợp đồng giữa các thành phần

**Status:** Accepted — thiết kế Stage 01 đã hoàn thành. Ở thời điểm đóng Stage 01 chưa có runtime; các stage sau đã triển khai shared types, validation, run API/SDK và các operation mở rộng. Xem [documentation index](../../docs/README.md) để đọc hiện trạng và verification, không dùng boundary lịch sử này làm mô tả runtime hiện tại.

**Phase:** A — Chốt thiết kế MVP

## Mục tiêu

Làm rõ các trao đổi cần cho MVP để SDK, backend, adapters và engine được triển khai tương thích mà vẫn giữ đúng ranh giới trách nhiệm.

## Phạm vi và đầu ra

- Mô tả các endpoint MVP, asynchronous submission, polling, lifecycle, lỗi và quy tắc gửi lại.
- Định nghĩa JSON Schema Draft 2020-12 cho cấu hình, observations/snapshots, model exchanges, engine outcomes và results/evidence.
- Ghép OpenAPI 3.1.0 tới cùng các schema; cố định version, timestamp UTC, decimal string và quy tắc thay đổi tương thích.
- Liên kết request/response bằng run, scenario, step, snapshot, inference, decision, order, fill, settlement và event IDs.
- Có ví dụ JSON hợp lệ, schema-invalid và schema-valid nhưng sai ngữ nghĩa để dùng làm fixture chung.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md): phạm vi, tài chính, thời gian và đầu ra mô hình đã được chốt cho MVP.
- HTTP/JSON là hướng Accepted; không suy ra rằng giao tiếp nội bộ Rust phải là HTTP.

## Quyết định đã chốt cho MVP 1

- `GET /healthz`, `POST /v1/runs`, `GET /v1/submissions/{submission_id}`, `GET /v1/runs/{run_id}`, và `GET /v1/runs/{run_id}/results`; chỉ health được triển khai ở Stage 02.
- Submit bất đồng bộ, lifecycle `queued → running → succeeded | failed`, Python client đồng bộ với polling; MVP không có cancellation hoặc streaming.
- `Idempotency-Key` UUID, equality theo parsed JSON (key object không quan trọng; thứ tự array và scalar values quan trọng), lookup sau khi mất acknowledgment, không tự retry POST/inference; dedup chỉ trong process backend.
- Snapshot inline; correlation theo inference/run/scenario/step/snapshot. Observation có event/availability time riêng; processing time không thay thế simulation time.
- Một output mode mỗi run: scores, targets hoặc market order intents. Response là proposal; admission, orders, fills, settlement, ledger và risk là các event riêng.
- Error envelope/code, local evidence references, OpenAPI/JSON Schema versioning và failure visibility theo các tài liệu liên quan.

## Open ngoài phạm vi Stage 01

- Authentication/authorization, hosted deployment, persistent database/queue, process-restart recovery, remote artifact retrieval và retention.
- Provider-specific adapters, real-market timestamps/terms, multi-currency data và các giao diện hủy/stream.
- Nội bộ Rust có thể dùng type/API trực tiếp; schema event không yêu cầu một HTTP hop giữa orchestrator và engine.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Endpoint run, polling và lifecycle | `docs(api): specify run operations and lifecycle` |
| 2 | Submission identity, mất acknowledgment, retry ownership và lỗi | `docs(api): specify submission identity and failures` |
| 3 | Instrument/run configuration schemas và constraints theo loại | `feat(contracts): define instrument configuration schemas` |
| 4 | Observation revisions, availability, snapshot và portfolio view | `feat(contracts): define temporal data and snapshot schemas` |
| 5 | Correlated model request và ba response variants | `feat(contracts): define correlated model exchanges` |
| 6 | Admission, order, fill, settlement, ledger và risk events | `feat(contracts): define engine outcome schemas` |
| 7 | Result summary, provenance và evidence references | `feat(contracts): define result and evidence schemas` |
| 8 | OpenAPI, versioning, compatibility, index và closeout | `docs(contracts): publish MVP contract compatibility rules` |

## Kiểm chứng dự kiến

- Kiểm tra JSON syntax, local `$ref`, link Markdown, fences, status, và các mẫu có kết quả số có thể tính độc lập.
- Đối chiếu rõ behavior producer/consumer trên mẫu schema-valid, schema-invalid và lỗi ngữ nghĩa; Stage 02 commit 25 thực hiện kiểm tra examples dùng chung qua JSON Schema, Rust và Python.
- Phân biệt timeout/retry vận hành khỏi hành động tài chính; không hứa exactly-once hoặc persistence khi chưa có bằng chứng.
- OpenAPI chỉ reference schema nguồn một lần; endpoint backtest không được gắn nhãn Implemented.

## Điều kiện hoàn thành

- API, SDK, adapter và engine có cùng field shape, ID, timestamp, lỗi và retry ownership cho MVP 1.
- OpenAPI/JSON Schema và ví dụ dùng chung khớp nhau; các kiểm tra fixture cross-language còn lại có điểm thực hiện riêng ở Stage 02.
- Tài liệu và schemas ở trạng thái Accepted về thiết kế; chưa mô tả API chạy được hoặc ổn định chỉ vì đã đặc tả xong.

## Tài liệu liên quan

- [Service contracts](../../docs/interfaces/service-contracts.md)
- [OpenAPI](../../contracts/openapi.json)
- [Compatibility rules](../../docs/interfaces/mvp-contract-compatibility.md)
- [Architecture overview](../../docs/architecture/overview.md)
