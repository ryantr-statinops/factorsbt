# Stage 10 — Nghiên cứu reranking có giới hạn

**Status:** Proposed

**Phase:** D — Nghiên cứu mở rộng

## Mục tiêu

Khám phá một bài toán xếp hạng lại candidate actions/policies trước portfolio admission, chỉ triển khai sau khi mục tiêu và hợp đồng được chấp nhận.

## Phạm vi và đầu ra

- Chọn một use case và baseline cụ thể; ghi inputs, mục tiêu đánh giá và giới hạn nghiên cứu.
- Định nghĩa candidate identity, thông tin được phép dùng, đầu ra xếp hạng và cách nối với admission policy hiện có.
- Giữ ranking tách khỏi quyết định vốn/thực thi của engine; thứ hạng không đồng nghĩa order hoặc fill được bảo đảm.
- Ghi evidence từ candidate set qua thứ hạng tới admission và kết quả; cập nhật contracts/ADRs nếu thay đổi ranh giới đáng kể.
- Cung cấp ví dụ bounded và báo cáo so sánh; không cam kết thuật toán sẽ cải thiện kết quả đầu tư.

## Phụ thuộc

- [Stage 08](08-scenario-comparison.md), [Stage 09](09-recorded-replay.md): comparison và provenance/replay đã được kiểm chứng.
- Đây là hướng Proposed độc lập với RL; không phải điều kiện hoàn thành MVP và không được coi là đã duyệt triển khai vì có file kế hoạch.

## Quyết định cần chốt — Open

- Xếp hạng action hay policy, nguồn candidate, objective/metric, baseline và quy tắc đánh giá.
- Thuật toán hoặc dịch vụ ranking; cách xử lý tie, conflict, thiếu candidate và failure nếu cần cho use case đã chọn.
- Lựa chọn/ranking/aggregation kết nối với shared capital ra sao; không mặc định voting hoặc cộng gộp nhiều model.
- Evaluation split, kiểm soát rò rỉ dữ liệu tương lai và model state; scope reproducibility của ranking response.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Chốt use case, objective và hợp đồng ranking | `docs(research): define bounded reranking experiment` |
| 2 | Triển khai ranking hook/adapter và evidence trong scope đã chốt | `feat(ranking): connect candidate ranking to portfolio admission` |
| 3 | Thêm ví dụ comparison và kiểm chứng ranh giới | `feat(research): demonstrate traceable reranking comparison` |

Task triển khai chỉ bắt đầu sau khi quyết định của task thiết kế được Accepted; không tự chọn thuật toán trong lúc code.

## Kiểm chứng dự kiến

- Candidate ranking chỉ dùng thông tin khả dụng tại simulation time và evaluation split đã chấp nhận.
- Thứ hạng khác nhau dẫn đến admission đúng policy khi thiếu vốn; engine vẫn thực thi constraints.
- Tình huống tie/conflict/failure được kiểm tra nếu thuộc contract đã chọn.
- So sánh baseline bằng cùng điều kiện đã khai báo; ghi cả trường hợp không cải thiện và bằng chứng replay theo scope.

## Điều kiện hoàn thành

- Một contract nghiên cứu được chấp nhận, một ví dụ chạy được và evidence kiểm chứng tương tác với portfolio/replay.
- Báo cáo kết quả và giới hạn theo metric đã định nghĩa; không suy rộng thành bảo đảm hiệu quả mô hình.

## Tài liệu liên quan

- [Product overview — longer-term research](../../docs/product/overview.md#longer-term-research-directions)
- [Roadmap — research](../../docs/roadmap.md#stage-3-reranking-and-reinforcement-learning-research)
