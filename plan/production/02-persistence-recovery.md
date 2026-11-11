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

## Kế hoạch thực thi chi tiết — 2026-11-11

**Status:** Proposed. Chọn một durable store và một mô hình worker đầu tiên sau P0. Các broker/database trong phần câu hỏi vẫn là ứng viên. [Quy tắc chung](README.md#quy-tắc-thực-thi-workstream-15) áp dụng cho mỗi task/commit và cổng integration.

### Phase P0 — Quyết định durability và compatibility

Phụ thuộc: RunStore/runtime hiện tại. Cổng: policy cho từng trạng thái, identity và write ordering được Accepted trước P1.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| P01 | docs(persistence): define restart and durability guarantees | Chốt queued/running/terminal sau restart cho run, replay, research và submission lookup. |
| P02 | docs(persistence): select the first durable store | Chọn storage local/team từ workload, concurrency, backup và deployment yêu cầu; không triển khai hai DB đồng thời. |
| P03 | docs(persistence): define durable identity and retry ownership | Idempotency lifetime, request equality, worker ownership, retry và cancellation scope. |
| P04 | docs(persistence): define evidence publication ordering | Chốt ghi evidence/manifest/terminal metadata, orphan handling và reconciliation khi crash giữa các bước. |
| P05 | docs(persistence): define checkpoint and version boundaries | Chốt restart-from-start, fail-interrupted hoặc resume; checkpoint schema và public contract nếu có thay đổi. |

### Phase P1 — Store có transaction và migrations

Phụ thuộc: P0. Vùng code: backend/run_store.rs, runtime.rs, admission/API và module store mới dự kiến. Cổng: semantics queue/idempotency hiện hành được bảo toàn trong adapter mới, transaction lỗi không tạo record nửa vời.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| P06 | refactor(backend): isolate job storage operations | Tách queue/identity/state transitions khỏi transport; giữ hành vi in-memory để đối chiếu. |
| P07 | feat(persistence): initialize the selected durable store | Config path/connection, quyền truy cập, health errors và lock theo P02. |
| P08 | feat(persistence): version store schema migrations | Tạo schema version; migration transaction, reject future schema và backup policy. |
| P09 | feat(persistence): persist submission identities atomically | Key/config equality và acknowledgement cùng transaction; race hai request cùng key. |
| P10 | feat(persistence): persist run lifecycle transitions | State machine và progress; reject transition sai hoặc terminal overwrite. |
| P11 | feat(persistence): persist replay lifecycle records | Source identity, report ref và lỗi; restart không mất lookup. |
| P12 | feat(persistence): persist research lifecycle records | Request/status/result ref; giữ failed-with-sealed-report khác evidence failure. |
| P13 | feat(persistence): restore queue admission capacity | Queue order/capacity và concurrent admission theo transaction; không oversubscribe. |
| P14 | test(persistence): verify durable admission concurrency | Integration duplicate/conflict/full queue với multiple connections và reopen store. |

### Phase P2 — Worker ownership và khôi phục sau crash

Phụ thuộc: P1. Cổng: kill/restart ở từng ranh giới không nhân đôi outcome; mọi job được phân loại theo policy P01. Không gọi bảo đảm exactly-once cho external model side effect nếu chưa có protocol tương ứng.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| P15 | feat(persistence): claim worker jobs atomically | Claim/lease hoặc single-process lock theo P03; hai worker không cùng publish một job. |
| P16 | feat(persistence): fence stale worker completions | Ownership generation bảo vệ stale completion sau takeover/restart. |
| P17 | feat(persistence): recover queued jobs on startup | Khôi phục FIFO đã chọn và đánh dấu queued hợp lệ; không tạo request identity mới. |
| P18 | feat(persistence): reconcile interrupted running jobs | Fail/restart/resume theo P01/P05; fresh inference mới không bị gọi là recorded replay. |
| P19 | feat(persistence): reconcile sealed evidence and metadata | Crash sau seal trước terminal publish; kiểm tra digest/identity, giữ orphan để chẩn đoán. |
| P20 | feat(persistence): publish terminal results durably | Result/ref và terminal status nhất quán; missing manifest không được success. |
| P21 | feat(persistence): drain workers with durable shutdown | Ngừng admission, xử lý lease và shutdown timeout theo policy; force-stop có dấu vết. |
| P22 | test(persistence): verify crash boundary recovery | Process-kill integration trước/sau claim, seal, publish; các loại job và model acknowledgement không rõ. |

### Phase P3 — Checkpoint có điều kiện

Phụ thuộc: P2, P05 Accepted có resume và các contract data/execution áp dụng đã ổn định. Nếu chọn fail-interrupted hoặc restart-from-start thì P23–P27 là Deferred và không chặn milestone đó. Cổng resume: kết quả tài chính và recorded inputs khớp uninterrupted run.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| P23 | feat(contracts): define complete simulation checkpoints | Portfolio, pending orders, clock, applied events, admission state, inference cursor, data/model/config identity. |
| P24 | feat(engine): snapshot at an atomic transition boundary | Chỉ checkpoint sau transition hoàn chỉnh; không serialize transient group staging. |
| P25 | feat(persistence): seal checkpoint generations atomically | Digest, executable/contract/data identity và complete marker; reject truncated generation. |
| P26 | feat(backend): resume compatible recorded execution | Restore engine/cursors, không gọi lại model cho inference đã recorded; incompatible checkpoint fail rõ. |
| P27 | test(persistence): compare resumed and uninterrupted jobs | Crash quanh fills/settlement/partial fills và model response; kiểm ledger/events không lặp. |

### Phase P4 — Vận hành store và nghiệm thu

Phụ thuộc: P2, thêm P3 nếu resume được chọn. Cổng: restart, migration, restore và retention được diễn tập trên dữ liệu synthetic; không xóa evidence còn được tham chiếu.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| P28 | feat(persistence): report storage and worker diagnostics | Readiness/store errors, queue depth, claims, reconciliation; redact secrets/payloads. |
| P29 | feat(persistence): back up consistent metadata references | Backup store và evidence theo cùng boundary; manifest inventory và ownership. |
| P30 | feat(persistence): validate restored job stores | Restore dry-run kiểm identity/digest/schema; không publish store restore chưa valid. |
| P31 | feat(persistence): plan reference-aware retention | Retention dry-run, active jobs/replay/checkpoint roots và shared refs được bảo vệ. |
| P32 | feat(persistence): apply bounded retention safely | Chỉ thực thi retention đã chốt; lock/race với active reader/writer, audit và quyền đường dẫn. |
| P33 | test(persistence): verify restart backup and restore workflows | SDK lookup/result sau restart, failed partial report, migration rollback, restore và disk-full. |
| P34 | docs(persistence): publish durable operations acceptance | Runbook setup/migrate/backup/restore/recovery; đo overhead, ghi durability và chưa hỗ trợ. |
