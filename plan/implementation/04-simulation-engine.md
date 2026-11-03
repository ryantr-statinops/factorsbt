# Stage 04 — Engine mô phỏng danh mục và thực thi

**Status:** Proposed

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Triển khai engine Rust có chuyển trạng thái danh mục giải thích được, tách quyết định, lệnh và khớp lệnh trong một danh mục nhiều tài sản dùng chung vốn.

## Phạm vi và đầu ra

- Quản lý đồng hồ mô phỏng, thứ tự sự kiện, tiền, vị thế và lệnh chờ theo phạm vi công cụ đã chốt.
- Cung cấp portfolio snapshot; xét toàn bộ decision group trên cùng trạng thái trước khi áp dụng thay đổi.
- Áp dụng chính sách vốn/rủi ro và chuyển đề xuất được chấp nhận thành order; ghi lý do từ chối hoặc giảm quy mô.
- Tạo fill theo điều kiện khớp, phí, trượt giá, latency và kế toán đã chốt; lưu state transition và kết quả từng bước.
- Tách engine khỏi HTTP, provider I/O và transport retries; mỗi scenario sở hữu trạng thái mutable riêng.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md), [Stage 01](01-contracts.md): quy tắc tài chính, thời gian và giao tiếp nội bộ đã chốt.
- [Stage 02](02-project-foundation.md), [Stage 03](03-data-snapshots.md): nền tảng Rust và đầu vào có identity/version.

## Quyết định đã Accepted cho Phase B

- Dùng event ordering và quy tắc kế toán trong [MVP 1 scope](../../docs/product/mvp-scope.md) cùng [acceptance fixtures](../../docs/examples/mvp/acceptance-cases.md). Quyết định ở close chỉ đủ điều kiện khớp từ open kế tiếp.
- Chỉ có market orders và full liquidity. Giảm/đóng vị thế được xét trước; tăng exposure dùng một hệ số vốn chung, 60 vòng tìm kiếm nhị phân và làm tròn lot về zero.
- Giá khớp lấy từ execution fixture, sau slippage, tick rounding và phí. Theoretical option marks không tạo execution price.
- Ledger dùng decimal checked arithmetic, làm tròn USD theo half-even đến cent; quantity làm tròn về zero theo lot. BS/CRR dùng `f64` chỉ để tính mark rồi chuyển một lần sang decimal 12 chữ số.
- Futures ban đầu lấy mark hợp lệ tại `start_at` làm settlement basis, không tạo fill hay P&L trước run. Không thêm partial fill, vay spot, FX, corporate actions hoặc netting.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Xây clock, portfolio state và xử lý sự kiện theo quy tắc | `feat(engine): add simulation clock and portfolio state` |
| 2 | Xét decision group và admission dùng chung vốn | `feat(engine): evaluate grouped decisions against shared capital` |
| 3 | Xử lý order/fill, kế toán và bằng chứng chuyển trạng thái | `feat(engine): simulate execution and record portfolio transitions` |

## Kiểm chứng dự kiến

- Hoán đổi thứ tự tài sản trong đầu vào không làm thay đổi phân bổ chỉ vì thứ tự vòng lặp.
- Hai đề xuất vượt vốn được giải quyết theo đúng chính sách và cùng snapshot; quyết định không được coi là fill.
- Dùng giá đóng bar không tạo khớp lệnh bất khả thi theo execution model đã chọn.
- Kiểm tra cash/positions, phí, rounding, lệnh chờ và sự kiện cùng timestamp bằng ví dụ tính tay trong phạm vi công cụ đã chốt.
- Khởi tạo hai scenario với cùng immutable data nhưng không chia sẻ trạng thái mutable.

## Điều kiện hoàn thành

- Engine chạy một chuỗi sự kiện controlled và cho đúng danh mục kỳ vọng cùng chuỗi decision/order/fill/transition.
- Các trường hợp cạnh tranh vốn, thời gian khớp và kết thúc run có bằng chứng kiểm chứng.
- Không có quy tắc tài chính quan trọng được quyết định ngầm trong code hoặc transport layer.

## Tài liệu liên quan

- [Simulation lifecycle](../../docs/architecture/simulation-lifecycle.md)
- [Rust engine role](../../.agents/roles/rust-engine.md)
