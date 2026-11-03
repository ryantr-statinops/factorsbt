# Stage 03 — Data adapters và snapshot theo thời điểm

**Status:** Implemented — local controlled fixtures and point-in-time snapshots; no real provider integration.

**Phase:** B — Backtest xuyên suốt

## Mục tiêu

Cung cấp cho mô phỏng đúng dữ liệu có thể biết tại thời điểm lịch sử, kể cả khi dữ liệu có nhiều tần suất và nhiều phiên bản.

## Phạm vi và đầu ra

- Triển khai adapter cho price bars và một nguồn sự kiện đã chọn, cùng fixture synthetic/controlled không phụ thuộc quyền truy cập provider.
- Giữ định danh tài sản, đơn vị, nguồn, version/revision, event time và availability time; processing time phục vụ vận hành được phân biệt riêng.
- Tạo snapshot có identity và phiên bản đầu vào rõ ràng; chỉ đưa vào dữ liệu đủ điều kiện ở đồng hồ mô phỏng.
- Phân biệt dữ liệu rỗng hợp lệ, thiếu coverage, metadata không đủ, quan sát cũ và lỗi provider.
- Lưu cấu hình/giả định availability đã chấp nhận; không tự suy ra lịch công bố từ kỳ báo cáo.

## Phụ thuộc

- [Stage 00](00-mvp-decisions.md), [Stage 01](01-contracts.md): nguồn dữ liệu, semantics thời gian và hợp đồng snapshot.
- [Stage 02](02-project-foundation.md): công cụ và nền tảng triển khai.

## Quyết định đã Accepted cho Phase B

- Dùng fixture cục bộ versioned, đăng ký khi backend khởi động; MVP không gọi nhà cung cấp dữ liệu thật.
- Mỗi observation giữ event time, availability time, nguồn, phiên bản và revision. Snapshot chỉ chọn revision khả dụng tại simulation time; snapshot đã tạo là bất biến.
- Tin tức có thể không có observation. Giá hoặc pricing input bắt buộc nếu thiếu, quá cũ hoặc chỉ khả dụng trong tương lai thì run lỗi rõ ràng; không carry-forward ngầm.
- Giá mở cửa để thực thi và sự kiện settlement là fixture inputs riêng, không đưa dữ liệu tương lai vào snapshot model.
- Dùng hash nội dung fixture để nhận diện dữ liệu bất biến. Không thêm cơ chế xác thực hoặc lưu trữ ngoài máy cục bộ trong MVP.

## Task → Commit

| # | Kết quả đã triển khai | Commit |
| --- | --- | --- |
| 01 | Đồng bộ trạng thái Phase A và các quyết định Phase B đã Accepted | `docs(plan): align Phase B with accepted MVP decisions` |
| 02 | JSON Schema Draft 2020-12 validator offline, có format checks | `feat(contracts): enforce shared wire validation` |
| 03 | Typed run configuration, observations, snapshots và portfolio | `feat(contracts): complete temporal and run configuration types` |
| 04 | Schema manifest/dataset nội bộ có version riêng | `feat(data): define versioned simulation fixture inputs` |
| 05 | Baseline và fixture variants cho thời gian, pricing, risk, settlement | `test(data): add controlled multi-instrument datasets` |
| 06 | Fixture registry, schema validation, identity và SHA-256 | `feat(data): load and validate immutable fixtures` |
| 07 | Availability/revision snapshot selection theo simulation time | `feat(data): assemble temporal decision snapshots` |
| 08 | Initial marks, execution opens và settlement events có time gate | `feat(data): expose execution and settlement inputs` |
| 09 | Coverage/freshness và phân loại dữ liệu missing, stale, unavailable, invalid | `feat(data): validate required simulation coverage` |
| 10 | Bằng chứng và trạng thái hoàn thành Stage 03 | `docs(data): record verified snapshot behavior` |

## Kiểm chứng đã chạy

- `cargo fmt --check` và `cargo test -p factorsbt-contracts -p factorsbt-data` pass: 19 contract tests (16 unit + 3 shared examples) và 16 data tests.
- `python -m unittest discover -s tests` trong `python-sdk` pass: 10 tests; kiểm tra toàn bộ fixture manifest, Draft 2020-12 schemas, ID/version pairing và digest.
- Rust validator dùng local schema registry, bật format validation và tắt network/file retrieval cho unresolved refs.
- Tests xác nhận late news/revision, fractional-second ordering, snapshot sorting/immutability, execution chỉ xuất hiện sau open và availability, settlement event timing, digest mismatch, path traversal, missing/stale/unavailable/invalid coverage.
- Fixture baseline có đủ bảy instrument IDs; fixture variants cung cấp risk recovery/failure, pricing reference và American settlement events.

## Điều kiện hoàn thành

- **Đạt cho phạm vi local fixture đã Accepted:** loader đăng ký fixture theo ID/version/digest; input được kiểm tra schema và identity trước khi dùng.
- Snapshot chỉ chứa revision đã available tại simulation time và không đổi khi một revision mới xuất hiện.
- Coverage xác nhận initial marks, close/pricing inputs và execution opens theo ngày; empty news hợp lệ, giá/pricing thiếu hoặc stale không được carry-forward.
- Engine/orchestrator có thể dùng snapshot, execution prices và settlement events có source identity/version rõ ràng.
- Provider thật, execution engine, model orchestration và API run chưa được triển khai trong Stage 03.

## Tài liệu liên quan

- [Simulation lifecycle](../../docs/architecture/simulation-lifecycle.md)
- [Service contracts](../../docs/interfaces/service-contracts.md)
