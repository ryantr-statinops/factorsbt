# Stage 08 — So sánh thí nghiệm giữa các scenario

**Status:** Proposed

**Phase:** C — Thí nghiệm và tái lập

## Mục tiêu

Cho phép thay đổi có kiểm soát dữ liệu, mô hình hoặc cấu hình mô phỏng và giải thích chênh lệch kết quả giữa các scenario độc lập.

## Phạm vi và đầu ra

- Mô tả thí nghiệm gồm baseline và các scenario có cấu hình/identity riêng; ghi rõ biến được thay đổi và phần giữ nguyên.
- Mỗi scenario sở hữu portfolio, pending orders, history, random state và model state mutable nếu có.
- Chỉ chia sẻ dữ liệu immutable bằng version/snapshot identity rõ ràng; không tái sử dụng phản hồi mô hình chỉ vì timestamp giống nhau.
- So sánh metric đã định nghĩa và các bước tạo ra chênh lệch, liên kết về input, quyết định và thực thi của từng scenario.
- Cập nhật SDK/backend/result contracts cho comparison và ảnh hưởng tới caller/kết quả lưu trữ.

## Phụ thuộc

- [Stage 07](07-python-sdk-mvp.md): MVP xuyên suốt và evidence của một scenario đã được xác minh.
- [Stage 03](03-data-snapshots.md), [Stage 05](05-model-orchestration.md): immutable snapshot identity và cơ chế cô lập model.

## Quyết định cần chốt — Open

- Cấu hình comparison, baseline, biến thí nghiệm và các metric/đơn vị/cách so sánh.
- Model isolation/reset; hành vi khi một scenario thất bại và cách biểu diễn kết quả chưa đủ.
- Có cần chạy song song hay không; nếu chọn, giới hạn concurrency và quy tắc giữ nguyên semantics.
- Chính sách cache/reuse chỉ được đưa vào khi điều kiện hợp lệ và identity đủ rõ; không đặt mục tiêu scale chưa đo.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Đặc tả comparison và tác động hợp đồng | `docs(contracts): define isolated scenario comparisons` |
| 2 | Triển khai scenario state và điều phối độc lập | `feat(experiments): isolate scenario execution state` |
| 3 | Công bố kết quả đối chiếu cùng chuỗi evidence | `feat(results): explain controlled scenario differences` |

## Kiểm chứng dự kiến

- So sánh price-only, price-plus-event và delayed-event bằng fixture; xác định quyết định nào thay đổi vì dữ liệu khác.
- Dùng cùng market timestamp nhưng portfolio khác phải tạo model input theo đúng scenario, không dùng nhầm response.
- Thay đổi/chạy lại một scenario không làm đổi state hay kết quả đã ghi của scenario khác.
- Stateful model và random state được reset/cô lập đúng chính sách; scenario thất bại không bị trình bày như kết quả đầy đủ.
- Nếu có parallel scheduling, đối chiếu với chạy tuần tự trong phạm vi reproducibility được chọn.

## Điều kiện hoàn thành

- Một bộ comparison controlled chạy được qua giao diện đã triển khai và giải thích được chênh lệch theo evidence.
- Có bằng chứng cách ly mutable state và input/response correlation; phạm vi share/cache được ghi rõ.
- Docs, SDK/backend và kết quả lưu trữ cùng phản ánh hợp đồng comparison mới.

## Tài liệu liên quan

- [Architecture overview](../../docs/architecture/overview.md)
- [Roadmap — comparative experiments](../../docs/roadmap.md#stage-2-comparative-experiments)
