# Stage 08 — So sánh thí nghiệm giữa các scenario

**Status:** Implemented — typed Python SDK runs independent v1 scenarios sequentially and creates a local comparison report from verified evidence.

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

## Hành vi và giới hạn đã xác minh

- `ComparisonSpec` nhận baseline và các variant có nhãn; `Client.compare()` chạy từng run độc lập theo thứ tự.
- Report giữ trạng thái từng scenario, metrics NAV/P&L/return, thay đổi cấu hình, overlap/unmatched timeline và trace differences liên kết artifact.
- Scenario bị từ chối, lỗi, chờ quá hạn hoặc không đọc được result được giữ trong report; việc đó không ngăn scenario tiếp theo.
- Không có comparison job/API riêng ở backend, chạy song song, cache hay tái sử dụng model response. Report được lưu cục bộ bằng SDK.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Thêm report calculation, timeline/trace và partial failure tests | `test(sdk): cover comparison report calculations`; `test(sdk): cover partial comparison failures` |
| 2 | Chạy baseline, price-only và delayed-event qua SDK/backend; xác nhận report và thứ tự tuần tự | `test(integration): verify controlled scenario comparison` |

## Kiểm chứng dự kiến

- Unit tests kiểm tra metric differences, interval, overlap/unmatched keys, configuration changes, trace links, lưu report và partial failures.
- Smoke test chạy tuần tự ba fixture qua backend thật; baseline và price-only có 28 điểm timeline chung, report có metrics và artifact links.
- Comparison chỉ claim các run độc lập tuần tự với model fixture stateless; chưa có kiểm chứng stateful model, random state hoặc parallel scheduler.

## Điều kiện hoàn thành

- Comparison chạy qua SDK/backend, report giữ kết quả từng run và liên kết timeline/trace tới evidence; unit và integration checks đã pass.
- Phạm vi local/sequential và việc không có cache hoặc parallel comparison được công bố rõ.

## Tài liệu liên quan

- [Architecture overview](../../docs/architecture/overview.md)
- [Roadmap — comparative experiments](../../docs/roadmap.md#stage-2-comparative-experiments)
- [Phase C local verification](../../docs/development/phase-c-local.md)
