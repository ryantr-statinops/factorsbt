# Stage 07 — Python SDK và nghiệm thu MVP xuyên suốt

**Status:** Proposed

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Đưa lát cắt MVP tới người dùng Python: cấu hình, gửi, theo dõi và đọc một backtest nhiều tài sản có kết quả giải thích được.

## Phạm vi và đầu ra

- Triển khai SDK thuần Python giao tiếp backend HTTP/JSON; simulation và inference không chuyển vào SDK.
- Cho phép đặt backend address, thiết lập kết nối và cấu hình truy cập theo contract đã chọn.
- Tách submit khỏi wait; biểu diễn validation, transport, external-service và execution failure với identity/ngữ cảnh cần thiết.
- Cung cấp ví dụ chạy thực tế với controlled fixtures và đường tích hợp data/model đã chọn; ghi rõ phần nào cần quyền truy cập bên ngoài.
- Trả kết quả liên kết cấu hình → snapshot → model response → admission → order → fill → portfolio; chỉ chọn metric khi định nghĩa đã được thống nhất.

## Phụ thuộc

- [Stage 01](01-contracts.md), [Stage 02](02-project-foundation.md): client contract và nền tảng Python.
- [Stage 06](06-backend-api.md) và các stage data/engine/orchestration phía dưới: đường chạy backend đầy đủ.
- Tiêu chí MVP và ranh giới kiểm chứng đã được chốt từ [Stage 00](00-mvp-decisions.md).

## Quyết định đã Accepted cho Phase B

- SDK tiếp tục dùng standard library và API đồng bộ; configuration và response được biểu diễn bằng typed builders/dataclasses.
- `submit(config, submission_id=None)` tạo UUID trước khi gửi nếu caller không truyền ID; lỗi acknowledgment không tự gửi POST lại và phải giữ submission ID để tra cứu.
- Cung cấp `get_submission`, `get_run`, `get_results` và `wait`. `wait` poll mỗi 0.25 giây, timeout cục bộ mặc định 60 giây bằng monotonic clock, không hủy run; run failure dùng typed exception.
- Lấy result là thao tác riêng sau khi run thành công. Không thêm metric ngoài những gì result contract đã định nghĩa.
- Phase B chỉ xác minh cách chạy package trong checkout/virtualenv; chưa công bố package hay thêm runtime dependency bên thứ ba.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Client configuration, submit và lỗi theo contract | `feat(sdk): add configurable backtest submission client` |
| 2 | Tracking/wait và đọc kết quả có evidence | `feat(sdk): track runs and retrieve traceable results` |
| 3 | Ví dụ xuyên suốt, kiểm chứng MVP và hướng dẫn đã chạy được | `feat(mvp): deliver verified Python-to-Rust backtest example` |

## Kiểm chứng dự kiến

- Đi từ SDK tới backend, data/model adapters và engine để thu kết quả danh mục controlled có kỳ vọng xác định trước.
- Bao phủ dữ liệu chưa khả dụng, revision, hai quyết định cạnh tranh vốn, fill eligibility, phí và quy tắc thực thi đã chốt.
- HTTP timeout lúc submit, duplicate effects và adapter failure truyền được qua SDK mà không báo thành công giả.
- Kiểm tra API Python qua hành vi công khai; ví dụ trong docs chạy đúng với interface thực tế, không còn nhãn conceptual khi đã xác minh.
- Ghi lệnh, kết quả và giới hạn kiểm tra; không suy rộng thành tuyên bố hiệu năng hoặc chất lượng provider.

## Điều kiện hoàn thành

- Người dùng chạy được một thí nghiệm xuyên suốt theo hướng dẫn và xem được nguyên nhân thay đổi danh mục.
- Toàn bộ acceptance cases cần cho MVP có bằng chứng theo ranh giới đã chấp nhận; phần chưa kiểm chứng được nêu rõ.
- Docs phản ánh đúng khả năng Implemented và phần Open. Đây là cổng hoàn thành Phase B/MVP, không đòi hỏi Phase C, D hoặc E.

## Tài liệu liên quan

- [Roadmap — first complete backtest path](../../docs/roadmap.md#stage-1-first-complete-backtest-path)
- [Python SDK role](../../.agents/roles/python-sdk.md)
- [Development workflow](../../docs/development/workflow.md)
