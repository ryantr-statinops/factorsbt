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

## Kế hoạch thực thi chi tiết — 2026-11-11

**Status:** Proposed. Thay đổi fill semantics là thay đổi hợp đồng tài chính: giữ v1 full-liquidity và phân version theo [compatibility policy](../../docs/interfaces/mvp-contract-compatibility.md). [Quy tắc chung](README.md#quy-tắc-thực-thi-workstream-15) áp dụng.

### Phase E0 — Chốt execution policy và dữ liệu cần dùng

Phụ thuộc: D03/D04 cho session/instrument. E0 xác định yêu cầu data fields để phối hợp D06/D10; E1 chỉ bắt đầu sau D1. Có thể thiết kế/test bằng synthetic trước khi adapter thực xong. Cổng: không dùng volume cả bar ở đầu bar khi volume đó chưa available.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| E01 | docs(execution): select the first liquidity workload | Chốt daily/intraday bars hay book; asset, size, fill frequency và workload benchmark. |
| E02 | docs(execution): define eligible price and volume timing | Quote/bar/volume availability, shared liquidity budget, zero/missing volume và no-lookahead. |
| E03 | docs(execution): define partial order lifecycle | Filled/remaining/canceled/expired, time-in-force, gap pricing, rounding và end-of-run remainder. |
| E04 | docs(execution): define collateral-safe group allocation | Policy khi chỉ một leg có liquidity; shared scale/bundle/reject, stable priority và rollback; không tạo naked delivery do leg kia chưa fill. |
| E05 | docs(execution): select cost and impact assumptions | Price basis, spread/slippage/impact, fee timing, calibration data và tránh double-count costs. |
| E06 | docs(execution): define versioned execution compatibility | Contracts, evidence/checkpoint/result counts, replay fingerprint và legacy full-fill behavior. |

### Phase E1 — Contracts và state lệnh

Phụ thuộc: E0. Vùng code: contracts, engine/orders.rs, backend/orchestrator.rs, SDK responses. Cổng integration: new order lifecycle roundtrip được và v1 vẫn giữ behavior cũ.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| E07 | feat(contracts): define versioned execution policies | Schema policy/config, defaults explicit, shared valid/invalid examples. |
| E08 | feat(contracts): define partial fills and remaining quantities | Order/fill identity, cumulative filled, fees, remainder và invariant signed quantity. |
| E09 | feat(engine): track pending order lifecycle quantities | State chưa fill/đã fill, lot step và terminal transitions; chống double fill. |
| E10 | feat(data): expose eligible liquidity observations | Đọc price/volume từ D1 reader với provenance; future/missing volume không thành budget ngầm. |
| E11 | feat(engine): reset shared liquidity budgets by event | Budget theo instrument/venue/event đã chọn; nhiều group không dùng lặp volume. |
| E12 | test(execution): verify lifecycle contract roundtrips | Contract → engine → evidence/SDK, legacy regression và malformed quantity cases. |

### Phase E2 — Fill theo liquidity và bảo toàn portfolio

Phụ thuộc: E1. Cổng: fill quantities không vượt order/budget; group tài chính hợp lệ sau partial allocation, kết quả không phụ thuộc input iteration.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| E13 | feat(engine): cap fill quantities by available liquidity | Participation/lot rounding theo E02; volume thấp/zero và signed sell. |
| E14 | feat(engine): allocate shared liquidity deterministically | Stable ordering hoặc proportional allocation đã chọn; hoán vị input cho cùng kết quả. |
| E15 | feat(engine): preserve collateral across partial groups | Chặn unbacked call/put/future khi leg tài trợ không fill; fixture covered-call và đóng đồng thời. |
| E16 | feat(engine): recheck buying power for partial fills | Thực giá/costs của fill candidate, valid reductions và funding caps đồng thời. |
| E17 | feat(engine): post costs for actual executed quantities | Fees chỉ charged cho quantity thực fill; per-order minimum nếu được Accepted, không lặp khi nhiều fill. |
| E18 | feat(engine): commit partial transitions atomically | Portfolio, liquidity consumption, order state và ledger cùng commit hoặc cùng rollback. |
| E19 | feat(engine): carry or cancel unfilled remainders | Time-in-force/session/end-of-run/expiry theo E03; không fill lại lượng đã hoàn tất. |
| E20 | feat(engine): apply liquidation liquidity policy | Chốt xử lý thiếu liquidity khi forced liquidation; không giữ giả định luôn đóng toàn bộ. |
| E21 | test(execution): verify illiquid grouped execution | Integration gap/fees/spot-future-option/low volume/rollback/liquidation và no-lookahead. |

### Phase E3 — Calibration và order types có điều kiện

Phụ thuộc: E2. E22–E24 cần calibration dataset từ D2; E25–E27 chỉ thực hiện nếu scope E01 chọn loại lệnh đó. L2/L3 và queue-position cần quyết định/schema/data riêng, không được coi là hệ quả tự động của phase này.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| E22 | feat(execution): validate versioned cost calibration inputs | Dataset/sample assumptions/units/version; fixture không chứa dữ liệu cấm phân phối. |
| E23 | feat(engine): apply the selected impact model | Chỉ một model đã chọn, bounded price và failure policy; cost sensitivity expected-value tests. |
| E24 | test(execution): compare calibrated and baseline fills | Đo fill ratio, cost và NAV sensitivity; không tuyên bố exchange fidelity từ synthetic. |
| E25 | feat(contracts): define selected limit and stop semantics | Trigger/eligibility/time-in-force và ambiguous intrabar ordering được Accepted trước code. |
| E26 | feat(engine): evaluate selected conditional orders | Limit/stop theo E25, same-bar high/low không tự suy thứ tự; cancellation/trigger fixtures. |
| E27 | test(execution): verify conditional order boundaries | Gap-through, partial fill, trigger then expire, unknown order type và missing quote. |

### Phase E4 — Backend, SDK, replay và nghiệm thu

Phụ thuộc: E2, thêm E3 nếu được chọn. P3 nếu dùng checkpoint phải được cập nhật cho remaining orders và liquidity cursor trước khi tuyên bố resume hỗ trợ execution mới. Cổng: cùng data/policy/binary tái lập financial transitions và remainder.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| E28 | feat(backend): orchestrate versioned execution policies | Clock events và errors theo E0; không đổi v1 paths ngầm. |
| E29 | feat(evidence): record liquidity and partial fill decisions | Requested/admitted/filled/remainder, budget inputs, cost/impact version và source refs. |
| E30 | feat(replay): verify partial execution transitions | So sánh fill sequence, consumed budgets, pending state và terminal failure. |
| E31 | feat(sdk): expose execution configuration and outcomes | Typed policy/order/fill/result, errors và compatibility rejection. |
| E32 | feat(comparison): compare execution cost and fill differences | Trace khác biệt fill/remainder/cost theo available evidence; thiếu dữ liệu hiện unavailable. |
| E33 | test(execution): verify SDK partial fills and offline replay | End-to-end new policy, failed group, offline model và legacy smoke. |
| E34 | perf(execution): measure liquidity allocation overhead | Workload E01 với số orders/instruments/fills, wall time/memory; so baseline có cùng inputs. |
| E35 | docs(execution): publish realistic execution acceptance | Hướng dẫn policy, calibration limits, fill assumptions và bằng chứng integration/benchmark. |
