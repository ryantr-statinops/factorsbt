# Stage 03 — Data adapters và snapshot theo thời điểm

**Status:** Proposed

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

## Quyết định cần chốt — Open

- Mapping provider, chuẩn hóa công cụ/đơn vị, timestamp và lịch giao dịch trong tập dữ liệu MVP.
- Fetch hay ingestion, storage/version identity, revision selection và trường hợp thiếu publication metadata.
- Carry-forward, freshness, dữ liệu thiếu và bar completion; quy tắc nào thuộc snapshot, quy tắc nào thuộc trigger của orchestrator.
- Phạm vi quyền truy cập và lưu dữ liệu; sử dụng secret reference theo cơ chế được chấp nhận, không chọn ngầm authentication.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Thêm dữ liệu chuẩn hóa, metadata và fixture theo hợp đồng | `feat(data): add versioned observations and controlled fixtures` |
| 2 | Triển khai adapter đầu tiên và phân loại lỗi/coverage | `feat(data): integrate MVP price and event adapters` |
| 3 | Xây snapshot theo availability và ghi provenance | `feat(data): assemble point-in-time snapshots` |

## Kiểm chứng dự kiến

- Báo cáo có event time tháng 3 nhưng công bố tháng 4 chỉ được nhìn thấy từ availability đã khai báo.
- Revision công bố sau không thay thế giá trị lịch sử trong snapshot trước đó; bar chưa hoàn thành không lộ giá đóng cửa.
- Hai nguồn khác tần suất được kết hợp theo chính sách freshness/missing-data đã chọn.
- Provider lỗi, metadata thiếu và empty response không bị gộp thành cùng một kết quả thành công.
- Snapshot có thể xác định lại đúng nguồn và phiên bản; log không chứa credentials hoặc payload riêng không được phép.

## Điều kiện hoàn thành

- Adapter và snapshot chạy được với fixture có kết quả kỳ vọng; đường provider được kiểm tra trong phạm vi truy cập thực có và ghi rõ phần chưa xác minh.
- Các ca availability/revision không lộ dữ liệu tương lai; giả định và giới hạn được tài liệu hóa.
- Engine và orchestrator có thể tham chiếu snapshot theo hợp đồng, không phải tự đoán dữ liệu đã khả dụng hay chưa.

## Tài liệu liên quan

- [Simulation lifecycle](../../docs/architecture/simulation-lifecycle.md)
- [Service contracts](../../docs/interfaces/service-contracts.md)
