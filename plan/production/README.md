# Kế hoạch mở rộng hướng production readiness

**Status:** Proposed. Đây là danh mục các khoảng trống đã được rà soát, chưa phải cam kết triển khai hay quyết định công nghệ.

**Mốc rà soát:** 2026-11-10. Repo hiện đã có các workflow synthetic chạy local và các phần mở rộng được xác minh trong [documentation index](../../docs/README.md). Danh mục này theo dõi những năng lực vượt ra ngoài phạm vi đó; không thay đổi trạng thái Implemented của MVP.

## Phạm vi

Mỗi hạng mục ghi nhận hiện trạng theo code/tài liệu, câu hỏi cần quyết định trước khi triển khai, và tiêu chí để một lát cắt tương lai có thể được xem xét hoàn tất. Các nhà cung cấp, định dạng, giao thức và hạ tầng được nêu là phương án cần đánh giá, chưa phải lựa chọn đã được chấp thuận.

| # | Workstream | Trạng thái | Phạm vi rà soát |
| --- | --- | --- | --- |
| 1 | [Dữ liệu thực tế và định dạng](01-data-layer.md) | Proposed | Provider adapters, columnar data, corporate actions, futures rollover |
| 2 | [Lưu trữ bền vững và phục hồi](02-persistence-recovery.md) | Proposed | Metadata, queue/worker, restart recovery và checkpoint |
| 3 | [Market microstructure](03-market-microstructure.md) | Proposed | Order book, partial fills, slippage và market impact |
| 4 | [Model transport và inference](04-model-inference.md) | Proposed | RPC/IPC và embedded inference |
| 5 | [Quant analytics và tearsheet](05-analytics-reporting.md) | Proposed | Performance metrics và báo cáo trực quan |
| 6 | [Triển khai, CI/CD và phân phối](06-delivery-and-licensing.md) | Proposed | Containers, CI, license và PyPI |

## Cách dùng

1. Chọn một workstream theo mục tiêu sản phẩm và người dùng dự kiến.
2. Chốt các quyết định Open ảnh hưởng đến dữ liệu, tài chính, hợp đồng hoặc vận hành trước khi phụ thuộc vào chúng trong code.
3. Chia workstream thành lát cắt có thể kiểm chứng; cập nhật trạng thái và dẫn chứng khi có thay đổi.
4. Chỉ chuyển sang Implemented khi hành vi có code và bằng chứng verification phù hợp. Một đề xuất hoặc ADR không tự nó chứng minh tính năng đã hoàn tất.

Không có thứ tự triển khai, deadline, scale target, provider guarantee hay cam kết production nào được quyết định trong tài liệu này. Quyết định kiến trúc đáng kể cần được ghi lại theo [quy tắc dự án](../../AGENTS.md) và liên kết với tài liệu sở hữu tương ứng trong [docs](../../docs/README.md).
