# Stage 09 — Recorded-response replay và provenance

**Status:** Implemented — the local API and typed SDK replay sealed runs with the same executable/contract, without fresh model inference.

**Phase:** C — Thí nghiệm và tái lập

## Mục tiêu

Phát lại chuyển trạng thái mô phỏng từ bằng chứng đã lưu trong một phạm vi được xác minh, đồng thời nhận diện rõ run gọi mô hình mới.

## Phạm vi và đầu ra

- Định nghĩa và lưu manifest gồm cấu hình, data/snapshot versions, ordering, model responses, random seeds khi áp dụng và thông tin engine/policy cần thiết.
- Replay tiêu thụ phản hồi đã ghi; fresh inference đi qua lời gọi mô hình và ghi provenance riêng.
- Kết nối manifest với admission, order, fill và state transitions để kiểm tra kết quả phát lại.
- Phát hiện thiếu/không tương thích evidence và báo giới hạn; không tự tải dữ liệu hiện tại thay phiên bản lịch sử hoặc âm thầm gọi model mới.
- Ghi ảnh hưởng của manifest/versioning tới artifact đã lưu, backend và SDK.

## Phụ thuộc

- [Stage 07](07-python-sdk-mvp.md), [Stage 08](08-scenario-comparison.md): đường chạy hoàn chỉnh và state độc lập theo scenario.
- Evidence được ghi từ data/engine/orchestration cần được rà soát và bổ sung trước khi công bố replay hoạt động.

## Quyết định và giới hạn đã chốt

- Chỉ đọc evidence đã niêm phong, có integrity-checked manifest/artifacts, contract `1.0.0` và executable digest giống binary đang chạy.
- So sánh theo giá trị có thứ tự cho admission, order, execution/fill, settlement, ledger, cash, positions và NAV; báo nhóm mismatch đầu tiên.
- Replay dùng recorded responses hoặc tái hiện terminal model failure; không gọi endpoint model và không fallback sang inference mới.
- Replay job dùng bounded FIFO queue chung với run; metadata/idempotency còn trong process, evidence replay nằm trong thư mục riêng cục bộ.
- Run nguồn thất bại vẫn có thể xác minh nếu replay tạo lại cùng stage/reason. Nguồn thất bại trước khi có summary được hỗ trợ; nguồn có summary phải có đủ financial evidence artifacts.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Bổ sung recorded inference, status/report API và typed replay SDK | `refactor(orchestrator): support recorded inference provider`; `feat(api): execute and expose recorded replay jobs`; `feat(sdk): add typed replay operations` |
| 2 | So sánh financial transitions, yêu cầu evidence đầy đủ và lưu evidence cô lập | `feat(replay): compare normalized financial outcomes`; `fix(replay): compare admission and ledger transitions`; `fix(replay): require complete financial evidence` |
| 3 | Kiểm tra idempotency, source failure, restart, queue, panic và model-call isolation | `test(integration): replay sealed source after backend restart`; `test(replay): cover admission queue and panic failures`; `test(integration): assert replay bypasses model service` |

## Kiểm chứng dự kiến

- Smoke chạy lại source từ thư mục evidence sau khi restart backend; ba output mode đều matched.
- Model fixture request count không tăng khi replay run thành công hoặc replay nguồn model-failure.
- Smoke kiểm tra report/artifact replay integrity, key lặp lại cùng request, conflict khi đổi source, và evidence gốc sau replay.
- Unit/integration tests kiểm tra source thiếu artifacts, queue overflow/FIFO, cùng UUID giữa run và replay, panic terminalization, comparator mismatch, và nguồn failure.
- Kết quả chỉ xác minh cùng executable digest/contract; không khẳng định reproducibility giữa build, platform hay fresh inference.

## Điều kiện hoàn thành

- Replay API/SDK hoạt động với run success và terminal model failure; status/report và replay evidence có identity riêng.
- Financial transition comparator, source integrity, same-binary rule, local evidence và không-fresh-inference được kiểm tra/công bố.
- Các Rust/Python và SDK/backend smoke checks ghi ở [Phase C local verification](../../docs/development/phase-c-local.md) đã qua.

## Tài liệu liên quan

- [Simulation lifecycle — replay](../../docs/architecture/simulation-lifecycle.md#replay-and-fresh-inference)
- [Service contracts](../../docs/interfaces/service-contracts.md)
- [Phase C local verification](../../docs/development/phase-c-local.md)
