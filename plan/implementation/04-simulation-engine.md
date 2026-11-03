# Stage 04 — Engine mô phỏng danh mục và thực thi

**Status:** Implemented — engine rules and the controlled multi-instrument MVP path are locally verified; focused derivative edge cases also have unit/reference coverage.

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Triển khai engine Rust có chuyển trạng thái danh mục giải thích được, tách quyết định, lệnh và khớp lệnh trong một danh mục nhiều tài sản dùng chung vốn.

## Phạm vi và đầu ra

- Đồng hồ synthetic và portfolio state riêng theo run; ledger dùng decimal checked arithmetic, tiền làm tròn half-even và futures lấy start mark làm settlement basis.
- Định giá quote/Black–Scholes/CRR với provenance; snapshot valuation yêu cầu input đúng simulation time, không carry-forward mark cũ.
- Chuyển scores, targets và market intents thành target/order proposals; scores Top K giải tie theo instrument ID.
- Xét target group trên shared capital; xử lý giảm vị thế trước, scale tăng exposure chung trong 60 vòng và làm tròn về lot.
- Tạo lệnh chỉ khớp ở open tiếp theo; recheck vốn theo giá open thực tế và áp dụng fill, phí, slippage, tick rounding.
- Đánh giá maintenance/collateral, hủy lệnh chờ, lên kế hoạch thanh lý toàn danh mục và giữ state nguyên vẹn nếu không có giá hoặc tài khoản không phục hồi.
- Kết thúc run bằng cách hủy pending orders, báo vị thế còn mở và giữ nguyên failure/last-valid state; engine không tạo fill tổng kết.
- Tách engine khỏi HTTP, provider I/O và retries; orchestration kết nối chuỗi xử lý ở Stage 05.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md), [Stage 01](01-contracts.md): quy tắc tài chính, thời gian và giao tiếp nội bộ đã chốt.
- [Stage 02](02-project-foundation.md), [Stage 03](03-data-snapshots.md): nền tảng Rust và đầu vào có identity/version.

## Quyết định đã Accepted cho Phase B

- Dùng event ordering và quy tắc kế toán trong [MVP 1 scope](../../docs/product/mvp-scope.md) cùng [acceptance fixtures](../../docs/examples/mvp/acceptance-cases.md). Quyết định ở close chỉ đủ điều kiện khớp từ open kế tiếp.
- Chỉ có market orders và full liquidity. Giảm/đóng vị thế được xét trước; tăng exposure dùng một hệ số vốn chung, 60 vòng tìm kiếm nhị phân và làm tròn lot về zero.
- Giá khớp lấy từ execution fixture, sau slippage, tick rounding và phí. Theoretical option marks không tạo execution price.
- Ledger dùng decimal checked arithmetic, làm tròn USD theo half-even đến cent; quantity làm tròn về zero theo lot. BS/CRR dùng `f64` chỉ để tính mark rồi chuyển một lần sang decimal 12 chữ số.
- Futures ban đầu lấy mark hợp lệ tại `start_at` làm settlement basis, không tạo fill hay P&L trước run. Không thêm partial fill, vay spot, FX, corporate actions hoặc netting.

## Task → Commit đã thực hiện

| Commit | Kết quả |
| --- | --- |
| 11–18 | Khởi tạo portfolio, ledger, synthetic clock, spot/futures/options accounting, margin/collateral và physical settlement |
| 19–21 | Pricing input validation, Black–Scholes European và CRR American |
| 22–24 | Mark/provenance, scores → Top K → targets, target và market-intent validation |
| 25–26 | Shared-capital scaling và execution group tại next open với giá fixture |
| 27–29 | Maintenance breach, whole-portfolio liquidation và end-of-run finalization |
| 30 | Engine acceptance path qua controlled fixture, quyết định spot, admission, open fill, mark, risk check và finalization |

## Kiểm chứng đã chạy

- `cargo fmt --all` hoàn tất; `cargo test -p factorsbt-engine --offline` pass: 52 unit tests và 1 controlled acceptance integration test.
- `cargo clippy --workspace --all-targets --offline -- -D warnings` pass.
- Unit tests bao phủ decimal ledger, margin/delivery, spot/futures/options accounting, European/physical option settlement, Black–Scholes/CRR references, shared capital, next-open eligibility, maintenance cancellation, liquidation recovery/failure và end-of-run state.
- Integration test xác nhận Top K → target weights → close-time admission → next-open gap recheck/fills → close marks → maintenance → finalization; thứ tự input không làm thay đổi kết quả admission trong test riêng.
- Acceptance evidence dùng fixture synthetic; không chứng minh provider thật, HTTP model call, service orchestration, stored evidence hay API/SDK run flow.

## Điều kiện hoàn thành

- Các thành phần engine được gọi xuyên suốt trong controlled spot acceptance test; phần giao tiếp model, scheduler/orchestrator và ghi evidence được xác minh ở Stage 05.
- Accounting, pricing, collateral, capital admission, next-open execution, liquidation và finalization có unit/reference evidence trong engine crate.
- Các giả định tài chính quan trọng theo MVP được giữ trong contracts/docs; implementation không thêm provider hay transport policy.

## Giới hạn đã xác minh

- Engine primitives được nối vào simulation runner/orchestrator và HTTP model fixture trong Stage 05–07. End-to-end acceptance smoke bao phủ ba output modes; các quy tắc futures/options, pricing, margin, settlement, capital admission và liquidation còn được kiểm tra ở unit/reference tests riêng, không suy rộng thành end-to-end cho mọi nhánh instrument.
- Phase B lưu evidence local và cung cấp run API/SDK, nhưng chưa có recorded replay, scenario comparison, provider thật, market connectivity, database, restart recovery, hosted service hoặc public API stability.
- Kiểm tra vốn tại open dùng execution prices; các fill vẫn tính phí/slippage/tick rounding. Nếu chi phí thực tế khiến nhóm không thể hoàn tất, candidate group thất bại nguyên tử và portfolio gốc không đổi.

## Tài liệu liên quan

- [Simulation lifecycle](../../docs/architecture/simulation-lifecycle.md)
- [Rust engine role](../../.agents/roles/rust-engine.md)
