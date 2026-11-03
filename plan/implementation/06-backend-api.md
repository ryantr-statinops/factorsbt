# Stage 06 — Backend API và quản lý run

**Status:** Implemented

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

## Quyết định đã Accepted cho Phase B

- Giữ bốn operations và lifecycle đã đặc tả trong [HTTP API](../../docs/interfaces/mvp-http-api.md); client đồng bộ poll trạng thái, không streaming/cancellation.
- Backend dùng metadata và deduplication in-memory tới khi process dừng; evidence lưu cục bộ và không tự xóa. Không có recovery sau restart.
- Mặc định hai run active và tối đa 16 queued, cấu hình lúc khởi động; worker queue xử lý FIFO.
- Kiểm tra submission identity/config atomically: cùng key và config trả acknowledgment ban đầu; cùng key khác config conflict. Queue đầy trả `503` và không tạo record mới.
- Chưa có authentication, hosted deployment, artifact download hay pagination trong MVP local.

## Task → Commit

| # | Kết quả đã commit | Commit |
| --- | --- | --- |
| 39 | Queue limits và process-local store | `d06d0ff feat(backend): configure bounded concurrent run execution` |
| 40 | Validate schema, fixture, coverage và initial portfolio | `70eb319 feat(api): validate submitted run configurations` |
| 41 | Atomic submission identity và equality | `1aef221 feat(api): deduplicate concurrent run submissions` |
| 42 | Worker pool, FIFO queue và active-slot dispatch | `003f3c8 feat(backend): schedule isolated concurrent runs` |
| 43 | Lifecycle, progress và failure resources | `52ec27e feat(api): expose run lifecycle and progress` |
| 44 | Successful result và evidence validation | `6d9f802 feat(api): expose verified run results` |
| 45 | Error envelope, request ID và input redaction | `ab5b562 feat(api): return structured contextual failures` |
| 46 | Local startup, worker failure cleanup và graceful drain | `1f48bab feat(backend): manage local worker lifecycle` |
| 47 | API race, queue, concurrent execution, failure và evidence tests | `80a4e1f test(api): verify concurrent submission and execution behavior` |
| 48 | Đồng bộ contract docs và bằng chứng triển khai | `docs(api): record implemented MVP run operations` |

## Kiểm chứng đã chạy

- Toàn bộ 50 test của `factorsbt-backend` pass. Trong đó API integration test chạy qua loopback thật với hai run đồng thời và inference responses đảo thứ tự.
- Concurrent POST cùng key/config trả cùng acknowledgment và chỉ tạo một run; đổi config trả `409 SUBMISSION_ID_REUSED`.
- Queue overflow trả `503 BACKEND_UNAVAILABLE` không tạo record; retry key đã nhận vẫn trả acknowledgment cũ khi queue đầy.
- Schema-invalid, thiếu idempotency key, contract version không hỗ trợ, invalid resource ID, run chưa xong, run lỗi và submission/run không tồn tại có error code/status theo contract.
- Hai run thành công giữ riêng model identity/result/evidence; lỗi model tạo `failed` status với step và last-valid-state reference.
- `GET .../results` kiểm tra schema, đường dẫn, SHA-256, kích thước và manifest trên `evidence_root/runs/<run_id>` mỗi lần đọc. Test sửa bytes xác nhận API không trả result khi digest sai.
- `cargo clippy -p factorsbt-backend --all-targets --offline` và `cargo fmt --all -- --check` pass. CI chưa được cấu hình.

## Điều kiện hoàn thành

- Các thao tác run/results thực sự chạy được và được kiểm chứng qua Rust HTTP service với controlled model/data fixtures.
- Contract/OpenAPI phản ánh behavior đã triển khai; không công bố stable API hoặc exactly-once vượt bằng chứng.
- Metadata/deduplication chỉ ở process; evidence local không tự xóa; không restart recovery, cancellation, streaming, artifact download, authentication hay hosted deployment.
- Service chỉ bind loopback; defaults là hai active và 16 queued, có thể cấu hình khi khởi động.

## Tài liệu liên quan

- [Service contracts](../../docs/interfaces/service-contracts.md)
- [Backend API role](../../.agents/roles/backend-api.md)
