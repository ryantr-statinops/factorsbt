# 1. Dữ liệu thực tế và định dạng

**Status:** Proposed. Provider, format và quy ước thị trường còn Open.

## Hiện trạng đã rà soát

- Luồng backtest hiện tại đọc fixture synthetic từ manifest và dataset JSON cục bộ; registry kiểm tra schema, identity và digest.
- Chưa thấy connector cho nhà cung cấp/sàn bên ngoài hoặc reader Parquet, Arrow, DuckDB hay Feather.
- Instrument futures có expiry và engine có xử lý settlement; chưa thấy tự động rollover hợp đồng.
- Corporate actions như chia tách và cổ tức nằm ngoài scope MVP.

Tham chiếu: [fixture workflow](../../contracts/fixtures/README.md), [MVP scope](../../docs/product/mvp-scope.md), [data layer](../../rust/crates/data/src/lib.rs).

## Câu hỏi cần quyết định

- Thị trường, vendor, quyền sử dụng dữ liệu, loại tài sản, độ phân giải và lịch sử cần hỗ trợ là gì?
- Chuẩn hóa symbol/instrument, timezone, calendar, event time, availability time, revision và missing data ra sao?
- Cần ingestion online, truy vấn lịch sử hay cả hai? Dữ liệu được lưu và version theo đơn vị nào?
- Quy tắc corporate action và futures roll nào áp dụng, nguồn dữ liệu nào là authority, và giá/position được điều chỉnh thế nào?
- Chọn benchmark và workload nào để xác định có cần columnar storage/reader ngay không?

## Tiêu chí cho lát cắt triển khai

- Adapter ghi rõ nguồn, version, đơn vị, event time và availability time; lỗi, dữ liệu thiếu và dữ liệu rỗng phân biệt được.
- Snapshot không nhìn thấy quan sát hoặc revision trước thời điểm khả dụng.
- Quy tắc corporate action/roll được chốt cho instrument và được kiểm tra bằng ví dụ xác định.
- Thay đổi contract/data format mô tả được ảnh hưởng đến adapter, caller và kết quả đã lưu.

## Kế hoạch thực thi chi tiết — 2026-11-11

**Status:** Proposed. Đây là thiết kế task cho triển khai sau; chỉ phần viết kế hoạch được thực hiện trong đợt này. Quy tắc Git, test và dependency chung ở [mục lục](README.md#quy-tắc-thực-thi-workstream-15). Mỗi hàng là một task và một commit dự kiến, bao gồm unit test có ý nghĩa cho hành vi đó.

### Phase D0 — Chốt lát cắt dữ liệu

Phụ thuộc: baseline synthetic hiện có. Cổng: các quyết định thị trường, quyền dữ liệu và semantics cần cho D1 được Accepted; nếu chưa có quyền truy cập thì dùng dữ liệu redistributable đã chọn, không tự lấy dữ liệu bên ngoài.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D01 | docs(data): define first real-data research slice | Chốt asset, venue, độ phân giải, khoảng lịch sử, quy mô benchmark và người dùng đầu tiên. |
| D02 | docs(data): select provider and data usage boundary | So sánh nguồn, quyền tải/lưu/chia sẻ fixture, offline replay và secret references; ghi quyết định. |
| D03 | docs(data): define temporal and session semantics | Chốt timezone, calendar, DST, availability, revisions, thiếu dữ liệu và close/open eligibility. |
| D04 | docs(data): define instrument and adjustment identity | Chốt symbol mapping, currency, raw/adjusted prices, split/dividend và roll cho asset đã chọn; đánh dấu mục không áp dụng. |
| D05 | docs(data): choose storage and compatibility boundary | Đo workload JSON hiện tại; chốt reader/storage tối thiểu, migration và version contract/fixture. |

### Phase D1 — Hợp đồng và bộ chuẩn hóa offline

Phụ thuộc: D0. Vùng code dự kiến: contracts, rust/crates/data, Python configuration. Cổng integration: cùng dữ liệu hợp lệ/không hợp lệ được Rust và Python hiểu thống nhất; v1 fixtures vẫn đọc được.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D06 | feat(contracts): define versioned historical data references | Schema dataset/partition/provenance theo D05; shared valid/invalid examples. |
| D07 | feat(contracts): model temporal data revisions | Tách event/availability/revision/source identity; ca revision đến muộn. |
| D08 | feat(data): normalize instrument identities | Mapping provider symbol sang ID, đơn vị và currency; reject collision. |
| D09 | feat(data): normalize timestamps and sessions | Parse UTC/session theo D03; kiểm tra DST, boundary và ngày không giao dịch. |
| D10 | feat(data): validate historical price records | Kiểm tra OHLC/volume/tick, duplicates và ordering; không biến missing volume thành zero. |
| D11 | feat(data): select point-in-time revisions | Chọn revision theo availability; giữ snapshot cũ bất biến. |
| D12 | feat(data): expose data quality diagnostics | Báo gaps, duplicates và rejected rows với provenance an toàn. |
| D13 | test(data): verify normalized dataset compatibility | Integration schema → normalization → snapshot, gồm v1 regression và lỗi dữ liệu thiếu. |

### Phase D2 — Ingestion và lưu dataset bất biến

Phụ thuộc: D1, quyền truy cập D02. Chỉ reader/provider được chọn mới triển khai; Parquet/Arrow/DuckDB không được tự động thêm cùng lúc. Cổng: import lại có identity rõ, interrupted import không xuất bản dataset nửa chừng.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D14 | feat(data): isolate historical provider adapter | Interface fetch/normalize/error theo provider đã chọn; offline fake adapter. |
| D15 | feat(data): configure provider secret references | Cấu hình endpoint và secret reference; redact logs/errors, không lưu key trong evidence. |
| D16 | feat(data): fetch bounded historical pages | Pagination/range limits và timeout; fake server kiểm tra lỗi giữa các page. |
| D17 | feat(data): classify ingestion retry conditions | Retry có giới hạn cho download an toàn theo D02; không retry lỗi dữ liệu/authorization. |
| D18 | feat(data): persist immutable dataset partitions | Ghi partition theo format D05; roundtrip chính xác decimal/time. |
| D19 | feat(data): seal dataset manifests atomically | Digest/size/partition refs; interrupted write không tạo manifest hoàn chỉnh giả. |
| D20 | feat(data): deduplicate repeated imports | Cùng nguồn/version/range có kết quả xác định; revision mới không overwrite dataset cũ. |
| D21 | feat(data): read versioned historical datasets | Reader theo format đã chọn; phát hiện file thiếu, sai digest, path escape. |
| D22 | test(data): verify interrupted ingestion and offline reads | Integration fake provider → sealed dataset → restart → read; không cần credential trong suite. |

### Phase D3 — Sự kiện thị trường theo asset được chọn

Phụ thuộc: D04 và D2. Chỉ các nhánh áp dụng được Accepted mới thực thi; nếu không áp dụng thì ghi lý do và bỏ task, không tạo commit rỗng. Cổng: giá điều chỉnh, position và cash không double-count.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D23 | feat(contracts): define corporate action events | Split/dividend identity, effective/availability time và nguồn authority. |
| D24 | feat(engine): apply split quantity adjustments | Điều chỉnh quantity/cost basis/order theo quy tắc D04; bảo toàn giá trị theo fixture xác định. |
| D25 | feat(engine): post dividend cashflows | Ex/record/pay semantics đã chọn; cashflow đúng một lần, replay được. |
| D26 | feat(data): distinguish raw and adjusted histories | Ngăn dùng adjusted price cùng corporate-action cashflow gây tính hai lần. |
| D27 | feat(contracts): define futures roll schedules | Contract identity, roll trigger, execution quotes và provenance. |
| D28 | feat(engine): execute explicit futures roll transitions | Close/open hai hợp đồng với costs/margin; rollback khi thiếu quote/capacity theo policy. |
| D29 | test(data): verify market event accounting | Integration snapshot → action/roll → ledger → replay với trường hợp lỗi và boundary. |

### Phase D4 — Backend, SDK và nghiệm thu dữ liệu

Phụ thuộc: D2 và nhánh D3 áp dụng. Cổng: một thí nghiệm dữ liệu thực đã chọn chạy từ SDK, lưu đủ inputs để replay offline; benchmark được báo bằng số đo, không suy từ ngôn ngữ Rust.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D30 | feat(backend): admit registered historical datasets | Validate identity/coverage trước queue, không tải mạng ngầm trong simulation. |
| D31 | feat(backend): seal historical input provenance | Evidence giữ dataset/partition/source identity và bytes/reference theo policy đã chọn. |
| D32 | feat(sdk): configure historical dataset runs | Typed config và lỗi version/coverage; không đổi v1 callers ngầm. |
| D33 | feat(cli): expose the selected data import workflow | CLI import/validate/lookup theo contract đã Accepted; lỗi thiếu quyền/thiếu dữ liệu rõ. |
| D34 | test(data): verify historical backtests and offline replay | SDK → backend → engine với dataset cho phép phân phối; provider tắt vẫn replay được. |
| D35 | perf(data): measure the selected historical workload | Đo import/read/snapshot time, peak memory, file size trên workload D01; lưu environment. |
| D36 | docs(data): publish historical data acceptance evidence | Hướng dẫn reproducible import/run, quyền dữ liệu, coverage và hạn chế; cập nhật trạng thái chỉ khi đạt gate. |
