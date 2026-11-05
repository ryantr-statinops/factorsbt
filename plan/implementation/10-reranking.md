# Stage 10 — Reranking có kiểm soát

**Status:** Implemented — controlled synthetic fixtures and shared engine path

**Phase:** D — Nghiên cứu mở rộng

## Mục tiêu

Đánh giá một heuristic reranker cố định trên các target spot có giới hạn. Đây là thí nghiệm so sánh có thể tái lập, không phải khuyến nghị đầu tư hay cam kết cải thiện hiệu quả.

## Hợp đồng nghiên cứu đã chốt

- Mỗi mốc quyết định có tối đa ba ứng viên: giữ tiền mặt, phân bổ 50% vào `SPOT_A`, hoặc phân bổ 50% vào `SPOT_B`. Mỗi ứng viên có ID duy nhất, điểm gốc trong `[0, 1]`, target và thời điểm availability.
- Cùng candidate set và điểm gốc được dùng cho hai portfolio độc lập. Baseline chọn điểm gốc cao nhất. Reranker chọn theo `raw_score - 0.25 × sum(abs(target_weight - current_weight))` trên hai mã spot.
- `current_weight` lấy từ giá trị vị thế chia NAV của snapshot tại close. Phép tính dùng decimal. Khi điểm bằng nhau, chọn candidate có ID tăng dần.
- Candidate set rỗng, ID trùng, candidate chưa available hoặc NAV không dương làm nhánh tương ứng thất bại với lỗi có cấu trúc.
- Chỉ target của một candidate được chọn chuyển qua admission hiện tại. Engine vẫn áp dụng giới hạn vốn; lệnh đủ điều kiện khớp sớm nhất ở open kế tiếp.
- Report ghép hai nhánh theo simulation time và gồm NAV cuối, P&L USD, tổng giá trị khớp, lựa chọn, trace candidate → requested/admitted target → fill, delta P&L và hướng cao hơn/thấp hơn/bằng baseline. Lỗi một nhánh không xóa kết quả nhánh kia.
- Dùng fixture versioned riêng cho phát triển và đánh giá. Hệ số và tie-break được cố định trước khi chạy fixture đánh giá. Báo cáo mức tăng, giảm hoặc bằng baseline mà không tuyên bố thuật toán tốt hơn.

## Phụ thuộc và giới hạn

- Dựa trên snapshot, admission, execution và evidence của MVP; không đổi quy tắc tài chính hiện có.
- Runner sở hữu state độc lập cho từng nhánh. Fixture là input bất biến có ID/version rõ ràng.
- Không huấn luyện hoặc gọi model ranking bên ngoài. Không hỗ trợ candidate tự sinh từ dữ liệu tương lai.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Chốt protocol reranking bounded, baseline, objective và metric | `docs(research): define bounded reranking protocol` |
| 2 | Trích bước mô phỏng dùng chung cho runner nghiên cứu | `refactor(engine): extract reusable simulation step` |
| 3 | Đăng ký fixture spot versioned cho development và evaluation | `feat(research): register versioned spot fixtures` |
| 4 | Kiểm tra candidate identity, target, score và availability | `feat(ranking): validate point-in-time candidates` |
| 5 | Chọn baseline và turnover-aware candidate bằng decimal | `feat(ranking): score baseline and turnover variants` |
| 6 | Chạy portfolio độc lập và tạo comparison report có trace | `feat(ranking): run isolated portfolio comparisons` |
| 7 | Nghiệm thu candidate, vốn, execution và partial failure | `test(ranking): verify controlled candidate selection` |

## Điều kiện hoàn thành

- Fixture development và evaluation cho thấy ít nhất một mốc chọn khác nhau.
- Mỗi target đi qua valuation, admission, pending-order execution, ledger fee, fill và close mark của engine; không tạo fill trực tiếp từ ranking.
- Kết quả từng nhánh, trace và metric được lưu; không có dữ liệu chưa available trong ranking.
- Kết quả đánh giá có thể tăng, giảm hoặc bằng baseline và được báo đúng.

## Bằng chứng triển khai

- Runner và report: [`reranking.rs`](../../rust/crates/backend/src/research/reranking.rs), fixture development/evaluation: [`fixtures.rs`](../../rust/crates/backend/src/research/fixtures.rs).
- Acceptance: [`research::tests`](../../rust/crates/backend/src/research/mod.rs), toàn bộ lệnh và giới hạn ở [Phase D local verification](../../docs/development/phase-d-local.md).
