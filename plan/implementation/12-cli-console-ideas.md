# Mục 12 — Brainstorm CLI tool / console application

**Status:** Proposed

**Phase:** E — Brainstorm ứng dụng, chưa chốt

## Ý tưởng cần ghi nhớ

Khám phá một CLI tool hoặc console application để tương tác với FactorsBT. Đây là nơi giữ ý tưởng để quay lại thảo luận; chưa xác định sản phẩm, giao diện hay phạm vi triển khai.

## Động cơ có thể khám phá

- Thao tác với thí nghiệm từ terminal khi không muốn viết một script mới cho mỗi lần sử dụng.
- Tìm hiểu nhu cầu tự động hóa thao tác hoặc quan sát run trong quy trình nghiên cứu.
- Xác định liệu một ứng dụng console có giá trị ngoài SDK hay chỉ lặp lại chức năng sẵn có.

Các điểm dưới đây là gợi ý brainstorm, không phải requirements, backlog đã duyệt hoặc danh sách tính năng được hứa hẹn.

## Hướng sử dụng có thể thảo luận

- Chuẩn bị/gửi một thí nghiệm từ cấu hình và xem lỗi đầu vào.
- Theo dõi run, xem lỗi và tìm kết quả/evidence.
- Khám phá cách trình bày comparison hoặc replay trong terminal nếu nhu cầu thực tế xuất hiện.

## Câu hỏi còn mở — Open

- Người dùng nào cần ứng dụng này, công việc nào SDK hiện tại chưa giải quyết thuận tiện?
- CLI không tương tác, console tương tác hay hình thức khác phù hợp với vấn đề đã xác định?
- Phạm vi tính năng, input/output, workflow, ngôn ngữ, thư viện, phân phối và môi trường hỗ trợ là gì?
- Nếu có scripting, cần bảo đảm gì về output, lỗi và compatibility? Chỉ bàn chính sách cụ thể khi use case được chọn.
- Sử dụng SDK hay gọi backend bằng cách khác; cấu hình endpoint và quyền truy cập ra sao? Chưa thay đổi kiến trúc API-first đã Accepted.
- Có nên thực hiện không, ưu tiên vào lúc nào và bằng tiêu chí nào để đánh giá giá trị?

## Quan hệ với các phase khác

- Không yêu cầu Phase D hoàn thành; vị trí Phase E chỉ phân nhóm ý tưởng, không xác lập lịch.
- MVP và trải nghiệm sử dụng SDK có thể cung cấp thông tin cho thảo luận, nhưng chưa ấn định dependency triển khai.
- Không phải điều kiện hoàn thành MVP; không đưa mục này vào tỷ lệ hoàn thành 12 stage triển khai.

## Cách quay lại ý tưởng

Khi có nhu cầu cụ thể, thảo luận mục tiêu và phương án trước. Nếu quyết định theo đuổi, tạo kế hoạch riêng có scope, dependency, contracts, Task → Commit và acceptance criteria. File brainstorm này chưa có kế hoạch code, commit triển khai, deadline hoặc tiêu chí nghiệm thu ứng dụng.

## Tài liệu liên quan

- [Product overview](../../docs/product/overview.md)
- [Architecture overview](../../docs/architecture/overview.md)
