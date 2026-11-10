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
