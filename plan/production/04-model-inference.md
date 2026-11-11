# 4. Model transport và inference

**Status:** Proposed. HTTP/JSON là giao thức ban đầu đã Accepted; giao thức độ trễ thấp và inference nhúng còn Open.

## Hiện trạng đã rà soát

- Backend gọi model qua HTTP/JSON; repo có HTTP fixture để kiểm tra luồng.
- Chưa thấy gRPC, Arrow Flight, shared-memory IPC, ONNX Runtime hoặc LibTorch được tích hợp.
- Model response được correlate và validate, sau đó vẫn phải qua admission/execution của engine; inference failure được biểu diễn riêng.

Tham chiếu: [ADR 0001](../../docs/decisions/0001-python-sdk-rust-backend.md), [model client](../../rust/crates/backend/src/model_client.rs), [model exchange contract](../../docs/interfaces/model-exchanges.md).

## Câu hỏi cần quyết định

- Workload nào cần độ trễ/thông lượng khác HTTP/JSON, và benchmark nào chứng minh overhead là nút thắt?
- Model chạy như service riêng hay trong process backend? Ngôn ngữ, runtime, thiết bị và cách cấp tài nguyên nào cần hỗ trợ?
- Payload/schema, version identity, timeout, cancellation, batching và tương thích replay ra sao?
- Cách cô lập state, giới hạn tài nguyên và xử lý lỗi model nhúng thế nào?
- Native runtime có làm thay đổi build matrix, phân phối wheel/binary hoặc license obligations không?

gRPC, Arrow Flight, shared memory, ONNX Runtime và LibTorch là phương án để nghiên cứu, chưa được chọn.

## Tiêu chí cho lát cắt triển khai

- Có benchmark tái lập so sánh với transport hiện tại trên workload đại diện.
- Correlation, version provenance, timeout và lỗi vẫn hiển thị rõ ở API/evidence.
- Kết quả không làm thay đổi semantics thời gian mô phỏng, admission, portfolio hay recorded replay ngoài hợp đồng đã chọn.
- Build, deploy và quyền sử dụng runtime được tài liệu hóa cho môi trường đích.

## Kế hoạch thực thi chi tiết — 2026-11-11

**Status:** Proposed. Giữ HTTP/JSON làm baseline. Chỉ triển khai một transport/runtime bổ sung khi benchmark và quyết định I0 chứng minh nhu cầu; nếu baseline đã đạt mục tiêu thì hoàn tất I0/I1, ghi kết quả và defer nhánh I2. [Quy tắc chung](README.md#quy-tắc-thực-thi-workstream-15) áp dụng.

### Phase I0 — Workload, semantics và quyết định đầu tư

Phụ thuộc: model client/guard/orchestrator hiện tại; D0 nếu workload dùng historical data. Cổng: xác định overhead ở đâu, chọn tối ưu có đo đạc; không mặc định HTTP là bottleneck.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| I01 | docs(inference): define model workloads and latency budgets | Kích thước snapshot, số decision, concurrent runs, target throughput/latency/memory và hardware đo. |
| I02 | docs(inference): define model state and identity ownership | Stateless/stateful boundary, reset/isolation, model content identity/version và non-deterministic output. |
| I03 | perf(inference): measure HTTP serialization and service costs | Tách encode/network/inference/decode/validation, cold/warm và tail latency trên fake model. |
| I04 | docs(inference): select one measured transport strategy | Giữ HTTP hoặc chọn một trong RPC/IPC/embedded sau benchmark, dependency/license/build cost và failure isolation. |
| I05 | docs(inference): define deadline and execution compatibility | Timeout/cancel/batching, simulated-time semantics, protocol version và source replay requirements. |

### Phase I1 — Ranh giới provider và HTTP baseline

Phụ thuộc: I0. Vùng code: backend/model_client.rs, inference_guard.rs, orchestrator.rs, runtime.rs. Cổng: refactor không đổi financial outputs và không phát sinh inference retry.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| I06 | refactor(inference): isolate provider exchange boundaries | Encode/transport/decode/validation tách rõ; giữ exact request/response bytes. |
| I07 | feat(inference): configure explicit provider capabilities | Protocol/model identity và state policy theo I02; reject unsupported capabilities. |
| I08 | feat(inference): bound response sizes and resource usage | Giới hạn payload/concurrency theo I01; lỗi oversized response không lộ body private. |
| I09 | feat(inference): propagate request deadlines | Deadline/cancel ownership và timeout errors; không thay đổi simulation clock ngầm. |
| I10 | feat(inference): isolate model state between runs | Session/reset mechanism nếu stateful được chọn; nếu không thì reject stateful mode. |
| I11 | feat(inference): expose redacted timing diagnostics | Latency breakdown/queue pressure, request IDs; không log secrets hoặc raw snapshots. |
| I12 | feat(examples): add a standalone controlled model service | Demo fixture launch độc lập, versioned response và shutdown; không nhập API test làm public SDK. |
| I13 | test(inference): verify baseline transport isolation | Integration concurrent runs, timeout, stale/corrupt/oversized reply và guard; so baseline ledger. |

### Phase I2 — Một transport/runtime được chọn, có điều kiện

Phụ thuộc: I04 Accepted và I1. Những hàng dưới là kết quả chung cho phương án được chọn, không phải yêu cầu cài tất cả runtime. Nếu embedded, handle lifecycle/resources là runtime; nếu RPC/IPC, là connection/process lifecycle. Cổng: parity với HTTP và rõ hành vi crash/cancel.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| I14 | build(inference): integrate the selected runtime dependency | Pin version/features, build matrix, license obligations và reproducible setup. |
| I15 | feat(inference): map versioned model exchange payloads | Decimal/time/correlation mapping chính xác; shared roundtrip fixtures. |
| I16 | feat(inference): manage provider connection or runtime lifecycle | Startup readiness, resource ownership, health và cleanup của phương án I04. |
| I17 | feat(inference): execute isolated provider requests | Một request một correlated outcome; process/session isolation theo I02. |
| I18 | feat(inference): enforce alternative provider deadlines | Hung inference, cancel result, stale completion và resource release; failure hữu hình. |
| I19 | feat(inference): validate model artifact provenance | Model/version/content digest và config identity theo runtime; mutable endpoint không là fingerprint. |
| I20 | feat(inference): batch only compatible requests | Chỉ nếu I05 chọn batching; không gộp future steps hoặc leak portfolio giữa scenarios; nếu không chọn thì defer. |
| I21 | test(inference): verify alternative provider contract parity | Cùng controlled outputs cho HTTP và provider mới; model crash, malformed reply, concurrent isolation. |

### Phase I3 — Evidence, replay và nghiệm thu

Phụ thuộc: I1 và I2 nếu được chọn; P03/P18 cho restart policy, không tự bật model retry. Cổng: replay dùng recorded outputs, không phụ thuộc provider đang online hoặc model artifact còn tồn tại.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| I22 | feat(evidence): record versioned inference provenance | Protocol/runtime/model digest/session/config/deadline metadata; giữ raw exchange policy đã Accepted. |
| I23 | feat(replay): replay recorded provider outputs offline | Load recorded response/failure qua common validation; không gọi runtime/remote model. |
| I24 | feat(sdk): configure the selected model provider | Typed config và errors, version boundary; credentials chỉ reference, không nhúng evidence. |
| I25 | test(inference): verify provider-independent financial replay | SDK → backend với model tắt sau source run, matched failure và divergence. |
| I26 | perf(inference): compare selected provider against HTTP | Cùng hardware/dataset/output/concurrency; cold/warm/tail latency/memory và build cost. |
| I27 | docs(inference): publish provider setup and recovery | Run/demo/limits/state reset/timeout recovery, secrets và supported environment đã xác minh. |
| I28 | docs(inference): record transport acceptance decision | Ghi benchmark, parity, trade-offs; nếu không có lợi ích thì giữ baseline và đóng nhánh tối ưu có lý do. |
