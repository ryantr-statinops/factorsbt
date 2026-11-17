# 1. Dữ liệu thực tế và định dạng

**Status:** D0 decisions, D1 offline contracts/normalization, and D1a intraday clock/execution prerequisites are Implemented with local verification. D1a does not include a maintained authoritative calendar catalog or complete intraday run wiring. Provider ingestion and later run integration remain unimplemented. Massive account access and applicable data-use rights are unverified.

## Hiện trạng đã rà soát

- Luồng backtest hiện tại đọc fixture synthetic từ manifest và dataset JSON cục bộ; registry kiểm tra schema, identity và digest.
- Chưa thấy connector cho nhà cung cấp/sàn bên ngoài hoặc reader Parquet, Arrow, DuckDB hay Feather.
- Instrument futures có expiry và engine có xử lý settlement; chưa thấy tự động rollover hợp đồng.
- Corporate actions như chia tách và cổ tức nằm ngoài scope MVP.

Tham chiếu: [fixture workflow](../../contracts/fixtures/README.md), [MVP scope](../../docs/product/mvp-scope.md), [data layer](../../rust/crates/data/src/lib.rs).

## Câu hỏi còn Open ngoài lát cắt đầu tiên

- Massive account entitlement and use rights for the actual user/use case remain unverified; a different license may be required for organizational, shared, or commercial use.
- Chưa chốt permanent security identifiers, ticker changes ngoài năm mã đầu, universal tick/lot/fractional-share rules, hoặc multi-currency.
- Corporate-action adjustment and futures roll remain outside the first slice; quyết định lại nếu scope mở rộng.
- Các format/reader thay JSONL chỉ được xem xét khi D4 workload đo được bottleneck cụ thể.

## Tiêu chí cho lát cắt triển khai

- Adapter ghi rõ nguồn, version, đơn vị, event time và availability time; lỗi, dữ liệu thiếu và dữ liệu rỗng phân biệt được.
- Snapshot ghi tách event interval, assumed availability và ingestion time; historical snapshot không được tuyên bố là exact point-in-time reconstruction.
- Quy tắc corporate action/roll được chốt cho instrument và được kiểm tra bằng ví dụ xác định.
- Thay đổi contract/data format mô tả được ảnh hưởng đến adapter, caller và kết quả đã lưu.

## Kế hoạch thực thi chi tiết — cập nhật 2026-11-16

**Status:** Phase D0 design documentation is complete. Các lựa chọn và giới hạn được ghi trong [data slice](../../docs/data/README.md), [ADR 0004](../../docs/decisions/0004-massive-historical-rest.md), [temporal semantics](../../docs/data/time-and-session.md), [instrument identity](../../docs/data/instrument-identity.md), và [benchmark/storage](../../docs/data/storage-and-benchmark.md). Chưa có provider account entitlement check hoặc authenticated API request; do đó chưa có online data import và Phase D2 vẫn bị chặn bởi entitlement/license gate. Quy tắc Git, test và dependency chung ở [mục lục](README.md#quy-tắc-thực-thi-workstream-15). Mỗi task code ghép unit test có ý nghĩa; integration kiểm tra cuối phase.

### Phase D0 — Chốt lát cắt dữ liệu

Phụ thuộc: baseline synthetic hiện có. **Kết quả:** hoàn thành bằng năm commit tài liệu một-file mỗi commit. Đã chốt US-listed Nasdaq stocks, 1-minute regular-session bars, AAPL/MSFT/AMZN/GOOGL/META, 20 completed sessions, historical-snapshot availability assumption, intraday-only EOD flatten with configured costs, strict-later-bar-open eligibility, raw prices, và JSONL partitions. Massive Stocks Basic historical REST được chọn có điều kiện; entitlement và quyền sử dụng account cụ thể vẫn chưa xác minh. Không có API request đã xác thực hoặc dữ liệu provider nào được tải. D2 online ingestion bị chặn đến khi account access và quyền sử dụng phù hợp được xác nhận. Benchmark synthetic D05 đã đo và ghi trong tài liệu storage.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D01 | docs(data): define first real-data research slice | `docs/data/README.md`: chốt asset, venue, độ phân giải, 20 phiên, workflow SDK, giới hạn snapshot và intraday-only. |
| D02 | docs(data): select provider and data usage boundary | ADR 0004: chọn Massive historical REST có điều kiện; ghi rõ quyền cá nhân, không chia sẻ lại và entitlement chưa xác minh. |
| D03 | docs(data): define temporal and session semantics | `docs/data/time-and-session.md`: UTC intervals, America/New_York calendar, DST, snapshot availability, gaps, flatten và strictly-later open. |
| D04 | docs(data): define instrument and adjustment identity | `docs/data/instrument-identity.md`: XNAS symbols, USD, raw bars, corporate-action boundaries và assumptions còn Open. |
| D05 | docs(data): choose storage and compatibility boundary | `docs/data/storage-and-benchmark.md`: JSONL manifest/partitions được chọn sau benchmark synthetic 39k/390k/3.9m. |

### Phase D1 — Hợp đồng và bộ chuẩn hóa offline

Phụ thuộc: D0. Vùng code dự kiến: contracts, rust/crates/data, Python configuration. Cổng integration: cùng dữ liệu hợp lệ/không hợp lệ được Rust và Python hiểu thống nhất; v1 fixtures vẫn đọc được. D1a calendar behavior chỉ xuất hiện ở versioned intraday contract.

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

**D1 gate — passed locally:** Historical dataset, manifest, bar and revision schemas are registered for offline Rust validation. Typed references are exposed in Rust/Python; Rust normalization checks mappings, UTC one-minute intervals, snapshot availability, OHLCV values, point-in-time revisions, and safe quality diagnostics. Rust integration verifies the schema-to-normalization path and existing v1 fixture loading. See [D1 local verification](../../docs/development/data-d1-local.md). This does not implement partition storage, provider ingestion, or end-to-end SDK runs. Nine commits landed on `main`, including a timestamp normalization correction found during integration acceptance.

### Phase D1a — Intraday clock and execution prerequisites

Phụ thuộc: D0 và D1. Phase này đưa calendar/session behavior vào contract mới, trước ingestion online. Không đổi semantics của daily synthetic v1. Cổng integration ở cuối phase bao gồm DST, holiday, early close, missing bars và regression daily-v1.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D37 | refactor(engine): isolate session schedule from synthetic daily clock | Tách daily-v1 schedule để có thể thêm lịch intraday độc lập mà không đổi event sequence cũ; unit regression cho v1. |
| D38 | feat(engine): support versioned one-minute session events | Session clock theo America/New_York, UTC event instants, DST và calendar version; unit tests boundary mùa hè/mùa đông. |
| D39 | feat(data): validate intraday coverage and run bounds | Kiểm tra interval, session date, early close và coverage; phân biệt absent bar với import/request failure; không forward-fill. |
| D40 | fix(backend): enforce strictly later bar-open execution | Market order chỉ đủ điều kiện nếu bar start `>` decision time; equality không khớp cùng timestamp. |
| D41 | feat(engine): flatten positions at regular-session close | Flatten tất định ở final bar close với configured costs, hủy order còn pending và fail nếu required close thiếu. |
| D42 | test(backend): verify minute runs across session boundaries | Integration DST/holiday/early-close/missing-bar và legacy daily-v1; kiểm tra không giữ position/order qua đêm. |

**D1a gate — passed locally:** The versioned offline schedule drives a separate UTC one-minute clock; incomplete source requests and confirmed coverage gaps remain distinct; strict-later same-session order scheduling and atomic spot flattening use configured costs and require each final close. Synthetic acceptance covers DST offsets, an omitted holiday date, early close, missing intervals/final close, and daily-v1 event ordering. See [D1a local verification](../../docs/development/data-d1a-local.md). This phase provides prerequisites only: no authoritative maintained calendar catalog, provider ingestion, or SDK/API end-to-end intraday runs are claimed.

### Phase D2 — Ingestion và lưu dataset bất biến

Phụ thuộc: D1, D1a và quyền truy cập phù hợp theo ADR 0004. Chỉ reader/provider được chọn mới triển khai; Parquet/Arrow/DuckDB không được tự động thêm cùng lúc. Nếu entitlement hoặc license chưa xác minh, không gọi API và giữ D2 online ingestion blocked; dùng fake adapter cho offline checks. Cổng: import lại có identity rõ, interrupted import không xuất bản dataset nửa chừng.

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

Phụ thuộc: D1a, D2 và nhánh D3 áp dụng nếu được chọn cho scope mở rộng. Cổng: một thí nghiệm dữ liệu thực đã chọn chạy từ SDK, lưu đủ inputs để replay offline khi license cho phép; benchmark được báo bằng số đo, không suy từ ngôn ngữ Rust. D3 market-action handling không phải điều kiện cho first slice vì vị thế và signal state reset theo session; nếu scope thay đổi thì phải hoàn tất D3 trước acceptance tương ứng.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| D30 | feat(backend): admit registered historical datasets | Validate identity/coverage trước queue, không tải mạng ngầm trong simulation. |
| D31 | feat(backend): seal historical input provenance | Evidence giữ dataset/partition/source identity và bytes/reference theo policy đã chọn. |
| D32 | feat(sdk): configure historical dataset runs | Typed config và lỗi version/coverage; không đổi v1 callers ngầm. |
| D33 | feat(cli): expose the selected data import workflow | CLI import/validate/lookup theo contract đã Accepted; lỗi thiếu quyền/thiếu dữ liệu rõ. |
| D34 | test(data): verify historical backtests and offline replay | SDK → backend → engine với dataset cho phép phân phối; provider tắt vẫn replay được. |
| D35 | perf(data): measure the selected historical workload | Đo import/read/snapshot time, peak memory, file size trên workload D01; lưu environment. |
| D36 | docs(data): publish historical data acceptance evidence | Hướng dẫn reproducible import/run, quyền dữ liệu, coverage và hạn chế; cập nhật trạng thái chỉ khi đạt gate. |
