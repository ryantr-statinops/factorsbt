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
