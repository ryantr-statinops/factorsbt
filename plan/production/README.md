# Kế hoạch mở rộng hướng production readiness

**Status:** Proposed. Đây là danh mục các khoảng trống đã được rà soát, chưa phải cam kết triển khai hay quyết định công nghệ.

**Mốc rà soát:** 2026-11-10. Repo hiện đã có các workflow synthetic chạy local và các phần mở rộng được xác minh trong [documentation index](../../docs/README.md). Danh mục này theo dõi những năng lực vượt ra ngoài phạm vi đó; không thay đổi trạng thái Implemented của MVP.

## Phạm vi

Mỗi hạng mục ghi nhận hiện trạng theo code/tài liệu, câu hỏi cần quyết định trước khi triển khai, và tiêu chí để một lát cắt tương lai có thể được xem xét hoàn tất. Các nhà cung cấp, định dạng, giao thức và hạ tầng được nêu là phương án cần đánh giá, chưa phải lựa chọn đã được chấp thuận.

| # | Workstream | Trạng thái | Phạm vi rà soát |
| --- | --- | --- | --- |
| 1 | [Dữ liệu thực tế và định dạng](01-data-layer.md) | D0 complete; implementation not started | First private US equity minute-bar slice, provider adapter, immutable data, later corporate-action scope |
| 2 | [Lưu trữ bền vững và phục hồi](02-persistence-recovery.md) | Proposed | Metadata, queue/worker, restart recovery và checkpoint |
| 3 | [Market microstructure](03-market-microstructure.md) | Proposed | Order book, partial fills, slippage và market impact |
| 4 | [Model transport và inference](04-model-inference.md) | Proposed | RPC/IPC và embedded inference |
| 5 | [Quant analytics và tearsheet](05-analytics-reporting.md) | Proposed | Performance metrics và báo cáo trực quan |
| 6 | [Triển khai, CI/CD và phân phối](06-delivery-and-licensing.md) | Proposed | Containers, CI, license và PyPI |

## Cách dùng

1. Chọn một workstream theo mục tiêu sản phẩm và người dùng dự kiến.
2. Chốt các quyết định Open ảnh hưởng đến dữ liệu, tài chính, hợp đồng hoặc vận hành trước khi phụ thuộc vào chúng trong code.
3. Chia workstream thành lát cắt có thể kiểm chứng; cập nhật trạng thái và dẫn chứng khi có thay đổi.
4. Chỉ chuyển sang Implemented khi hành vi có code và bằng chứng verification phù hợp. Một đề xuất hoặc ADR không tự nó chứng minh tính năng đã hoàn tất.

Thứ tự triển khai đề xuất được phân rã bên dưới; các quyết định công nghệ/tài chính chưa được Accepted vẫn là gate trước code. Không có deadline, scale target, provider guarantee hay cam kết production được chốt. Quyết định kiến trúc đáng kể cần được ghi lại theo [quy tắc dự án](../../AGENTS.md) và liên kết với tài liệu sở hữu tương ứng trong [docs](../../docs/README.md).

## Quy tắc thực thi workstream 1–5

**Mốc cập nhật:** 2026-11-16. Phạm vi được người dùng chọn là workstream 1–5 trong thư mục này: dữ liệu, persistence, microstructure, model inference và analytics. Workstream 6 tiếp tục là danh mục riêng; chưa đưa CI/CD hay phát hành public vào chuỗi triển khai.

- Phase D0 được thực thi trực tiếp trên `main`, mỗi file một task/commit tài liệu; đích đã chọn là push một lần lên `origin/main` sau khi cổng D0 đạt. Không tạo PR hoặc merge riêng. Các thay đổi D0 không tải provider data hoặc dùng credentials.
- Mỗi hàng trong năm kế hoạch là một task có một mục đích review và một commit Conventional Commits dự kiến. Khi thực thi code, unit/regression tests của hành vi và tài liệu sở hữu đi cùng task; không tách commit test để che một khoảng không được kiểm chứng.
- Build tại cuối mỗi task code. Chạy integration/regression cuối mỗi phase; phase chưa đạt gate không được xem hoàn tất. Cuối chuỗi có nghiệm thu tổng. Task chỉ sửa Markdown kiểm tra links/anchors/fences/status/diff, không ép runtime build.
- D2 online ingestion is blocked until account entitlement and data-use rights are verified for the intended use. Do not test authenticated endpoints without an authorized account/credential or silently switch providers.
- Trước mỗi commit kiểm tra branch/worktree/index, stage explicit paths, đọc staged diff, chạy git diff --cached --check; sau commit kiểm tra file list và summary. Không stash/reset/amend/squash hay gom công việc có sẵn.
- Quyết định financial/product/storage/provider chưa Accepted là gate chặn code phụ thuộc. Viết task quyết định không tự chấp thuận lựa chọn. Nhánh có điều kiện chỉ làm khi được chọn; nhánh không áp dụng ghi Deferred/lý do, không tạo commit rỗng.
- Những schema/enum/simulation/retry semantics mới phải theo [compatibility policy](../../docs/interfaces/mvp-contract-compatibility.md); bảo toàn v1 và stored evidence, không thêm field vào strict schema v1 âm thầm. Recorded replay tiếp tục kiểm tra executable/contract, không hứa cross-binary replay.
- Chỉ dùng synthetic/redistributable fixtures trong tests; online acceptance cần quyền dữ liệu/credentials đã có và riêng biệt. Không cần provider account để chạy regression offline; không log/commit secrets hay dataset trái quyền.
- Đường dẫn module mới trong task là vùng sở hữu dự kiến, cần xác nhận layout trước code. Dependencies mới được giải thích trong task lựa chọn tương ứng; không cài tất cả công nghệ được liệt kê trong câu hỏi Open.

## Phân rã và điểm bắt đầu

| Workstream | Phases | Task / commit dự kiến | Vùng sở hữu chính |
| --- | ---: | ---: | --- |
| [1 — Data](01-data-layer.md#phase-d0-chốt-lát-cắt-dữ-liệu) | D0, D1, D1a, D2–D4: 6 phases | D01–D42: 42 | contracts, Rust data/engine, backend admission/evidence, SDK |
| [2 — Persistence](02-persistence-recovery.md#kế-hoạch-thực-thi-chi-tiết-2026-11-11) | P0–P4 | P01–P34: 34 | backend store/runtime, evidence, engine checkpoints |
| [3 — Execution](03-market-microstructure.md#kế-hoạch-thực-thi-chi-tiết-2026-11-11) | E0–E4 | E01–E35: 35 | contracts, data inputs, engine orders/costs, replay, SDK |
| [4 — Inference](04-model-inference.md#kế-hoạch-thực-thi-chi-tiết-2026-11-11) | I0–I3 | I01–I28: 28 | model client/guard/orchestrator, evidence, SDK/examples |
| [5 — Analytics](05-analytics-reporting.md#kế-hoạch-thực-thi-chi-tiết-2026-11-11) | A0–A4 | A01–A34: 34 | contracts, engine valuation, backend series/evidence, SDK/report |
| Tổng | 25 phases | 173 task/commit dự kiến | Bao gồm decision, integration, benchmark và docs tasks |

173 là số task trong bản phân rã workstream 1–5, không phải ước lượng effort, deadline hay số commit bắt buộc. Năm task G01–G05 bên dưới đưa tổng kế hoạch lên 178 task dự kiến. Số thực tế giảm nếu bỏ nhánh có điều kiện, hoặc tăng nếu quyết định sau này cần tách một thay đổi lớn. Không chia thành commit từng câu; thay đổi contract, producer và consumer cần một chuỗi phối hợp có thể review.

## Waves và dependency đề xuất

| Wave | Phases / thứ tự | Phụ thuộc và gate |
| --- | --- | --- |
| W0 — Quyết định | D0, P0, I0, A0; E0 sau temporal/instrument decisions của D0 | Chốt đủ semantics cho từng nhánh. Benchmark I03 thực hiện để quyết định có làm I2. |
| W1 — Đầu vào và state | D1 → D1a → D2; P1 → P2; I1; E1 sau D1; A1 sau D1 và E1 nếu chọn partial fills | Intraday calendar/clock integration phải đạt trước online ingestion. E1 có thể chạy trên synthetic đúng schema D1 trước real ingestion. D2 vẫn gated by provider entitlement and rights. |
| W2 — Financial behavior | D3 nếu asset yêu cầu; E2; A2 sau A1; I2 nếu I04 chọn | Không có real provider cũng test execution bằng fixtures; không gọi đó là calibration thực. Chốt shapes cuối trước checkpoint. |
| W3 — Liên thông và đầu ra | D4 sau D2 và D3 nếu được chọn; E3 nếu chọn → E4; I3; A3 nếu chọn → A4; P3 nếu resume được chọn → P4 | E4 baseline không yêu cầu checkpoint; hỗ trợ checkpoint cho partial execution chỉ được công bố sau P3. I3 dùng restart policy P2; P4 bao gồm backup/restore và cleanup ownership. |
| W4 — Nghiệm thu tổng | G01–G05 bên dưới | Các nhánh bắt buộc và nhánh được chọn đều có evidence; Deferred/Open được công bố. |

Các phase cùng Wave chỉ có thể làm độc lập khi đã đủ prerequisites ghi trong từng file; bảng này không tự cho phép delegation hoặc chạy mutation song song trên cùng worktree. Phase D0 đã hoàn thành. Điểm bắt đầu tiếp theo của data path là D1/D1a bằng synthetic inputs; chỉ mở online D2 sau khi provider entitlement và rights được xác nhận. Không bắt đầu toàn bộ 173 task cùng lúc.

## Nghiệm thu tổng — một task / commit dự kiến cho mỗi hàng

Các task G không nằm trong tổng 173 ở trên; **toàn bộ kế hoạch là 178 task dự kiến** khi giữ tất cả nhánh. Không sửa runtime chỉ để làm đẹp báo cáo nghiệm thu; lỗi phát hiện cần task fix riêng và kiểm tra lại gate bị ảnh hưởng.

| ID | Commit dự kiến | Phạm vi và điều kiện đạt |
| --- | --- | --- |
| G01 | test(acceptance): verify the selected research workflow | Data import → versioned run → partial/cost fills → metrics/report; independent ledger/NAV/cost recomputation trên fixture được phép dùng. |
| G02 | test(acceptance): verify durable replay and recovery | Restart/kill giữa queue, inference, fill và seal; không duplicate outcomes; model/provider offline khi recorded replay; conditional checkpoint equivalence. |
| G03 | test(acceptance): preserve legacy SDK and contract workflows | Existing v1 fixtures/run/replay/research/comparison/installed CLI smoke; version mismatch và source tamper fail rõ. |
| G04 | perf(acceptance): record integrated workload measurements | Dataset/read/engine/inference/report/persistence time, memory và evidence I/O theo workload đã chốt; không gán latency/speedup chưa đo. |
| G05 | docs(acceptance): publish reproducible research handoff | Clean-install walkthrough, support environment, task ledger, hashes, tests, benchmarks, Open/Deferred và local/remote/CI state. |

Commands hiện hành để baseline: [quickstart](../../docs/development/quickstart.md) và [verification record](../../docs/development/regression-verification-2026-11-09.md). Khi thêm tooling/runtime mới, ghi lệnh thực tế đã xác minh trước khi dùng làm gate. Tổng acceptance gồm Rust workspace tests, format/strict Clippy, Python/contracts suite, existing smoke programs, research example và integration mới của các nhánh đã chọn; dùng thư mục build/evidence tạm và không cần public release.

## Theo dõi hoàn thành và bàn giao

Mỗi phase khi bắt đầu tạo ledger theo task ID: Proposed/Accepted design, commit hash, kết quả build/unit/integration, environment, benchmark và giới hạn. Phân biệt trạng thái thiết kế với việc task đã đạt gate; chưa có task code nào trong bộ kế hoạch này được đánh dấu Implemented.

Kế hoạch ban đầu đã kiểm tra relative links/heading anchors và 172 task ID. Phase D0 hiện có năm task tài liệu đã commit; kiểm tra links/anchors/fences/status cuối phase sẽ ghi nhận kết quả trong đợt thực thi này. Đây là kiểm chứng cấu trúc tài liệu/plan, không phải kết quả chạy các tính năng tương lai.

Trước push nếu được chọn sau này, so remote base, không force-push; remote tiến hoặc push bị từ chối thì dừng push và báo. Khi CI có thật, theo dõi tới kết quả; CI failure dừng theo skill và báo run lỗi, không tự tạo follow-up fix. Nếu không có hoặc không truy cập được thì báo CI unavailable. Bàn giao danh sách task/commit theo phase và số nhánh Deferred, không suy mức hoàn tất từ số file kế hoạch.
