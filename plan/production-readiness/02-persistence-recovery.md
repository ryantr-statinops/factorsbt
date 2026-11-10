# 2. Lưu trữ bền vững và phục hồi

**Status:** Proposed. Backend local hiện tại không có durable metadata store hoặc crash recovery.

## Hiện trạng đã rà soát

- `RunStore` giữ queue và metadata submission/run/replay/research trong bộ nhớ của process.
- Evidence run được seal trên filesystem và có API đọc index/tải artifact đã xác minh. Evidence có thể đọc lại sau restart từ cùng evidence root; status và metadata trong `RunStore` không được khôi phục theo cách đó.
- Queue hiện là bounded và in-process; chưa thấy distributed broker/worker hoặc resume từ checkpoint.

Tham chiếu: [RunStore](../../rust/crates/backend/src/run_store.rs), [evidence access](../../docs/interfaces/results-evidence.md), [architecture boundaries](../../docs/architecture/overview.md).

## Câu hỏi cần quyết định

- Local mode cần SQLite hay filesystem journal là đủ? Team mode có nhu cầu DB/queue dùng chung không?
- Những trạng thái nào phải tồn tại qua restart: submission, run status, progress, replay, research job, result reference?
- Cần semantics nào cho worker lease, retry, idempotency, cancellation và job đang chạy khi process dừng?
- Checkpoint gồm những state nào, chụp tại ranh giới nào, và làm sao xác minh checkpoint tương thích với executable, contract, data và model response?
- Yêu cầu retention, backup, migration, concurrency và access control là gì?

SQLite, PostgreSQL, Redis, RabbitMQ hay giải pháp khác chỉ là lựa chọn để đánh giá; chưa được chấp thuận trong kế hoạch này.

## Tiêu chí cho lát cắt triển khai

- Sau restart, API thể hiện trạng thái job theo quy tắc đã công bố; evidence và metadata không mâu thuẫn.
- Job chưa hoàn tất được đánh dấu hoặc phục hồi theo chính sách đã chọn, không âm thầm báo thành công.
- Retry/recovery không nhân đôi tác động tài chính hoặc tạo hai kết quả cho cùng identity.
- Có migration/compatibility và failure-path evidence cho dữ liệu persistent.
