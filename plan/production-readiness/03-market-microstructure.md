# 3. Market microstructure

**Status:** Proposed. Execution assumptions hiện chỉ được xác nhận cho synthetic fixture.

## Hiện trạng đã rà soát

- Engine dùng execution price từ fixture; nhóm lệnh đủ điều kiện được xử lý với giả định full liquidity.
- Có phí theo bps/per-unit và slippage theo bps cấu hình trên instrument; đây không phải một mức phí/slippage cố định duy nhất.
- Chưa thấy L2/L3 order book, queue position, limit/stop order, partial fill theo volume hoặc mô hình market impact.

Tham chiếu: [MVP scope](../../docs/product/mvp-scope.md), [execution implementation](../../rust/crates/engine/src/execution.rs), [simulation lifecycle](../../docs/architecture/simulation-lifecycle.md).

## Câu hỏi cần quyết định

- Mục tiêu là nghiên cứu daily bars, intraday bars hay tick/order-book data? Nguồn và độ sâu dữ liệu nào có sẵn?
- Semantics của market/limit/stop order, priority, queue position, partial fill, cancel và expiry là gì?
- Volume nào được dùng làm liquidity budget, và xử lý nhiều order cạnh tranh cùng một snapshot ra sao?
- Slippage/market impact cần calibration theo dữ liệu nào? Almgren–Chriss hoặc square-root law chỉ là ứng viên cần đánh giá, không phải mặc định.
- Phí, tick/lot, venue rules và market calendar khác nhau được biểu diễn/version thế nào?

## Tiêu chí cho lát cắt triển khai

- Phân biệt proposal, order, execution/fill; fill có price, quantity, thời điểm và provenance rõ ràng.
- Partial fills và liquidity constraints bảo toàn thứ tự thời gian, vốn dùng chung và tính nhất quán portfolio.
- Tham số mô hình được ghi vào evidence để replay và so sánh có thể giải thích.
- Kiểm chứng với ví dụ có volume thấp, gap, cạnh tranh vốn và không có quote khả dụng.
