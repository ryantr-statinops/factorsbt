# Stage 00 — Chốt phạm vi và quyết định MVP

**Status:** Accepted — quyết định synthetic MVP đã được chốt; đây là stage thiết kế, không tự chứng minh runtime Implemented.

**Phase:** A — Chốt thiết kế MVP

## Mục tiêu

Chuyển hướng sản phẩm thành một lát cắt backtest đủ rõ để triển khai mà không tự chọn ngầm giả định tài chính hoặc sản phẩm.

## Phạm vi và đầu ra

- MVP đã chốt gồm công cụ synthetic dùng chung vốn, price bars và sự kiện có availability time, model HTTP fixture stateless và kết quả truy vết được; xem [phạm vi MVP](../../docs/product/mvp-scope.md).
- Lập ma trận quyết định: câu hỏi, phương án, trạng thái, lý do chọn, tài liệu sở hữu và stage bị ảnh hưởng.
- Định nghĩa thí nghiệm mẫu, dữ liệu synthetic/controlled, kết quả kỳ vọng và giới hạn không hỗ trợ.
- Đưa quyết định về đúng tài liệu sản phẩm, lifecycle hoặc contracts; thay đổi kiến trúc đáng kể được ghi bằng ADR, giữ lịch sử quyết định cũ.

## Phụ thuộc

- Đọc tài liệu nền tảng và ADR 0001. Hướng Accepted hiện có là SDK thuần Python, backend/engine Rust và giao tiếp ban đầu HTTP/JSON.
- Đây là điểm bắt đầu của chuỗi triển khai; chưa phụ thuộc code hay dịch vụ bên ngoài có sẵn.

## Quyết định đã chốt và phần còn Open

- Accepted: researcher workflow, spot/futures/options synthetic, USD và lịch open/close UTC; chủ sở hữu là phạm vi MVP.
- Accepted: fixture có identity/version và availability rõ ràng; provider thật, quyền truy cập và metadata lịch sử ngoài fixture còn Open.
- Accepted: scores, targets hoặc market order intents; conversion và grouped admission nằm trong contracts/decision policies.
- Accepted: thứ tự open/close, revision theo availability, không carry-forward ngầm, hủy lệnh chờ cuối run; xem lifecycle.
- Accepted: cash/margin/reserves, full execution có giới hạn affordability, phí/slippage và decimal rounding; xem accounting và margin risk.
- Accepted: service loopback, metadata process-local và evidence local. Verification của các phase đã thực thi nằm trong documentation index; lịch kiểm tra cho công việc mới vẫn cần chọn theo phạm vi.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Ghi phạm vi MVP, ví dụ nghiên cứu và phần loại trừ | `docs(product): define first executable MVP scope` |
| 2 | Ghi quyết định dữ liệu, thời gian, vốn và thực thi đã được chốt; ADR nếu cần | `docs(simulation): record MVP financial and time policies` |
| 3 | Xác lập fixture, tiêu chí chấp nhận và giới hạn MVP | `docs(plan): define MVP acceptance scenarios` |

Các thông điệp trên giữ lại breakdown thiết kế ban đầu, không phải yêu cầu tạo lại commit đã hoàn thành. Code và verification xuất hiện ở các stage tiếp theo.

## Kiểm chứng dự kiến

- Đối chiếu từng lựa chọn giữa product, lifecycle, contracts và ADR; Accepted không được hiểu thành Implemented.
- Đi bộ qua một ví dụ có báo cáo công bố muộn, hai đề xuất cạnh tranh vốn và một đề xuất không được khớp.
- Xác minh ví dụ có đủ giả định để xác định đầu ra kỳ vọng, không chứa dữ liệu riêng hoặc credentials.

## Điều kiện hoàn thành

- Những quyết định cần cho MVP được chấp nhận và ghi rõ; câu hỏi còn Open có giới hạn tác động, không bị code phụ thuộc sử dụng như mặc định.
- Phạm vi, ví dụ và tiêu chí nghiệm thu được thống nhất; chưa có tuyên bố SDK/API đang hoạt động.

## Tài liệu liên quan

- [Product overview](../../docs/product/overview.md)
- [Simulation lifecycle](../../docs/architecture/simulation-lifecycle.md)
- [ADR 0001](../../docs/decisions/0001-python-sdk-rust-backend.md)
