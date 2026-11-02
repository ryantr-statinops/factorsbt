# Stage 00 — Chốt phạm vi và quyết định MVP

**Status:** Proposed

**Phase:** A — Chốt thiết kế MVP

## Mục tiêu

Chuyển hướng sản phẩm thành một lát cắt backtest đủ rõ để triển khai mà không tự chọn ngầm giả định tài chính hoặc sản phẩm.

## Phạm vi và đầu ra

- Đề xuất MVP gồm nhiều tài sản dùng chung vốn, price bars cùng một nguồn sự kiện, một model service và kết quả truy vết được. Phạm vi này cần được chốt trước khi triển khai.
- Lập ma trận quyết định: câu hỏi, phương án, trạng thái, lý do chọn, tài liệu sở hữu và stage bị ảnh hưởng.
- Định nghĩa thí nghiệm mẫu, dữ liệu synthetic/controlled, kết quả kỳ vọng và giới hạn không hỗ trợ.
- Đưa quyết định về đúng tài liệu sản phẩm, lifecycle hoặc contracts; thay đổi kiến trúc đáng kể được ghi bằng ADR, giữ lịch sử quyết định cũ.

## Phụ thuộc

- Đọc tài liệu nền tảng và ADR 0001. Hướng Accepted hiện có là SDK thuần Python, backend/engine Rust và giao tiếp ban đầu HTTP/JSON.
- Đây là điểm bắt đầu của chuỗi triển khai; chưa phụ thuộc code hay dịch vụ bên ngoài có sẵn.

## Quyết định cần chốt — Open

- Người dùng và tình huống MVP; thị trường, loại công cụ, lịch giao dịch, đồng tiền và khoảng thời gian mẫu.
- Nhà cung cấp, nguồn sự kiện, quyền truy cập, phiên bản dữ liệu và cách xử lý thiếu availability metadata.
- Đầu ra mô hình: dự báo, tỷ trọng mục tiêu hay ý định đặt lệnh; chính sách chuyển đầu ra thành đề xuất có thể xét duyệt.
- Decision trigger, thứ tự sự kiện cùng timestamp, dữ liệu thiếu/cũ/revision và kết thúc run khi còn lệnh chờ.
- Phân bổ vốn, rủi ro, loại lệnh, điều kiện khớp, phí, trượt giá, độ trễ, làm tròn, độ chính xác số và kế toán công cụ.
- Phạm vi chạy ban đầu, yêu cầu truy cập và cách kiểm chứng code: theo task, cuối phase hoặc kết hợp. Chưa chọn lịch chạy test cho công việc tương lai.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Ghi phạm vi MVP, ví dụ nghiên cứu và phần loại trừ | `docs(product): define first executable MVP scope` |
| 2 | Ghi quyết định dữ liệu, thời gian, vốn và thực thi đã được chốt; ADR nếu cần | `docs(simulation): record MVP financial and time policies` |
| 3 | Xác lập fixture, tiêu chí chấp nhận và giới hạn MVP | `docs(plan): define MVP acceptance scenarios` |

Thông điệp trên dành cho công việc tương lai; không tạo các commit này trong lần soạn kế hoạch.

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
