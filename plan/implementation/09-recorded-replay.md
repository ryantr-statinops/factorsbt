# Stage 09 — Recorded-response replay và provenance

**Status:** Proposed

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

## Quyết định cần chốt — Open

- Manifest/schema, artifact identity và compatibility; engine/platform/version nào nằm trong phạm vi tái lập.
- So sánh chính xác hay dùng tolerance nào nếu cần, ở những trường nào và theo lý do kế toán/numeric đã chấp nhận.
- Storage, retention và khả năng giữ dữ liệu/phản hồi theo quyền sử dụng thực tế; cách báo artifact thiếu hoặc bị hạn chế truy cập.
- Cách lựa chọn replay/fresh inference và mô tả provenance trong SDK/API/results; endpoint hoặc seed không đủ chứng minh model identity.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Đặc tả phạm vi reproducibility và manifest | `docs(replay): define evidence manifest and reproducibility scope` |
| 2 | Lưu/đọc evidence có phiên bản và xử lý thiếu artifact | `feat(evidence): persist replay inputs and recorded responses` |
| 3 | Triển khai replay và phân biệt fresh inference qua giao diện | `feat(replay): reproduce recorded simulation transitions` |

## Kiểm chứng dự kiến

- Phát lại run controlled và đối chiếu decision admission, order/fill, cash/positions và transitions theo tiêu chuẩn đã chọn.
- Dịch vụ model không khả dụng trong replay vẫn không phát sinh inference request; dùng spy/fixture quan sát lời gọi.
- Thiếu response/data version hoặc artifact không tương thích tạo lỗi/giới hạn rõ ràng, không fallback âm thầm.
- Fresh inference với cùng input có thể nhận response khác và luôn mang provenance gọi mới.
- Replay từng scenario không ghi đè state/evidence của run gốc hoặc scenario khác.

## Điều kiện hoàn thành

- Replay được xác minh trên fixture và công bố chính xác phạm vi tái lập, không hứa determinism chung cho mọi môi trường/model.
- API/SDK/results phân biệt replay với fresh inference; manifest có identity và quy tắc tương thích được ghi rõ.
- Đủ bằng chứng hoàn thành Phase C: comparison có thể giải thích và replay được kiểm tra trong phạm vi đã tuyên bố.

## Tài liệu liên quan

- [Simulation lifecycle — replay](../../docs/architecture/simulation-lifecycle.md#replay-and-fresh-inference)
- [Service contracts](../../docs/interfaces/service-contracts.md)
