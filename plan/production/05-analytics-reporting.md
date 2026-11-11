# 5. Quant analytics và tearsheet

**Status:** Proposed. Phạm vi metric, benchmark và cách trình bày chưa được chốt.

## Hiện trạng đã rà soát

- SDK trả về typed run/result data; scenario comparison có basic metrics gồm initial/final NAV, PnL, return, cùng timeline/trace.
- Chưa thấy bộ metric Sharpe, Sortino, Calmar, drawdown duration, alpha/beta, information ratio, turnover hoặc exposure theo factor/sector.
- Chưa thấy Plotly, HTML tearsheet hoặc tích hợp QuantStats/PyFolio.

Tham chiếu: [SDK comparison metrics](../../python-sdk/src/factorsbt/comparison.py), [result contract](../../docs/interfaces/results-evidence.md).

## Câu hỏi cần quyết định

- Metric tính từ daily NAV, intraday NAV hay fill-level series? Tần suất annualization và risk-free rate nào?
- Benchmark, currency, return convention, cashflows, missing periods và treatment của unrealized PnL được xác định thế nào?
- Alpha/beta và information ratio cần benchmark nào? Exposure theo ngành/factor lấy taxonomy từ đâu?
- Cần API typed metrics, notebook visualization, export HTML hay nhiều đầu ra?
- Metric nào thuộc kết quả chuẩn đã seal, metric nào là post-processing có thể tính lại?

## Tiêu chí cho lát cắt triển khai

- Công thức, sampling frequency, annualization và assumptions được ghi rõ cùng metric.
- Kiểm tra bằng chuỗi NAV/benchmark có kết quả biết trước, gồm biên dữ liệu ngắn, drawdown và missing observations.
- Báo cáo nêu nguồn và identity dữ liệu; không ngụ ý hiệu suất tương lai hoặc độ chính xác ngoài giả định.
- Visualization truy ngược được tới dữ liệu đã xác minh và hoạt động theo đầu ra đã chọn.

## Kế hoạch thực thi chi tiết — 2026-11-11

**Status:** Proposed. Metric definitions và annualization là quyết định tài chính phải được Accepted trước tính toán. Giữ result v1 và bằng chứng lịch sử; schema mới theo [compatibility policy](../../docs/interfaces/mvp-contract-compatibility.md). [Quy tắc chung](README.md#quy-tắc-thực-thi-workstream-15) áp dụng.

### Phase A0 — Định nghĩa series, metric và đầu ra

Phụ thuộc: portfolio/results/evidence/comparison hiện có; có thể dùng golden synthetic series để thiết kế độc lập với real provider. Cổng: công thức, sampling, cashflow và benchmark được chốt bằng ví dụ tính tay.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| A01 | docs(analytics): select initial research metrics | Chọn bộ metric tối thiểu và câu hỏi người dùng; phân biệt required với optional nâng cao. |
| A02 | docs(analytics): define NAV return and cashflow conventions | Snapshot valuation point, initial NAV, external flows, simple/log return, unrealized PnL, currency. |
| A03 | docs(analytics): define sampling and annualization | Calendar/frequency, risk-free units, missing sessions, short/zero/negative NAV và degenerate statistics. |
| A04 | docs(analytics): define benchmark alignment rules | Dataset/version/currency/total-return policy, overlapping windows và không forward-fill ngầm. |
| A05 | docs(analytics): select metric ownership and report format | Backend sealed series vs SDK post-processing; schema/version, reproducible exports và dependencies cần thiết. |

### Phase A1 — Series tài chính có provenance

Phụ thuộc: A0; D03/D04 nếu dùng real sessions/actions, E08 nếu series chứa partial fills. Vùng code: backend/results/evidence, contracts, SDK responses/comparison. Cổng: NAV/cashflows/fills đối chiếu được với ledger, không dùng snapshot trước transition làm end-of-period NAV.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| A06 | feat(contracts): define versioned analytics input series | NAV/cashflows/timestamps/units/provenance và quality metadata; v1 không bị thêm field ngầm. |
| A07 | feat(engine): sample portfolio values at declared boundaries | Emit start/end và close valuation theo A02; failed run chỉ có completed valid points. |
| A08 | feat(evidence): seal analytics series and cashflows | Digest, sample identity, policy/config/source refs; không mutate source evidence để tính metric. |
| A09 | feat(sdk): load verified analytics inputs | Read bytes/reference theo API được chọn; replay/research không giả có run artifact routes. |
| A10 | feat(analytics): validate ordered comparable NAV series | Duplicate/nonfinite/missing interval, currency mismatch và incomplete series rõ ràng. |
| A11 | feat(analytics): align benchmark and portfolio sessions | Intersection/required coverage theo A04, nêu unmatched points; không dùng dữ liệu chưa available. |
| A12 | test(analytics): reconcile sealed series with portfolio ledger | Integration independent cashflow/NAV recomputation và artifact tamper, failed source coverage. |

### Phase A2 — Metrics cốt lõi tính độc lập

Phụ thuộc: A1 và A01–A04 Accepted. Mỗi task kèm expected-value tests tính độc lập, không sao chép công thức implementation vào test. Cổng: golden tables, invalid/degenerate inputs và units được đối chiếu.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| A13 | feat(analytics): compute cashflow-aware period returns | Returns theo A02 với external flows, zero NAV và missing interval. |
| A14 | feat(analytics): compute cumulative and annualized returns | Compounding và elapsed/sampling convention, short/nonpositive wealth behavior. |
| A15 | feat(analytics): compute drawdown depth and duration | Peak-to-trough/recovery, unrecovered final drawdown và flat series. |
| A16 | feat(analytics): compute volatility and Sharpe ratios | Sample statistics/risk-free conversion, zero variance và insufficient points trả unavailable theo contract. |
| A17 | feat(analytics): compute downside risk and Sortino ratios | Target return/downside denominator đã chốt; all-positive/flat series. |
| A18 | feat(analytics): compute Calmar ratios | Annualized return/drawdown convention; zero drawdown và short interval. |
| A19 | feat(analytics): compute turnover costs and exposure | Executed notional/portfolio convention, partial fills, futures/option notionals; không nhầm số orders với turnover. |
| A20 | feat(analytics): describe metric availability and quality | Value/unit/sample_count/window/assumptions và unavailable reason; không thay unavailable bằng zero. |
| A21 | test(analytics): verify core metrics against golden series | Bộ tính tay gains/losses/cashflows/drawdown/gaps và numerical tolerance đã chọn. |

### Phase A3 — Benchmark metrics và report có điều kiện

Phụ thuộc: A2. A22–A24 chỉ chạy khi A01/A04 chọn và benchmark đủ chất lượng; sector/factor exposure cần taxonomy/data riêng nên giữ ngoài core milestone nếu chưa chọn. Cổng: series aligned và report không làm người dùng tưởng metric có đầy đủ coverage khi dữ liệu thiếu.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| A22 | feat(analytics): compute benchmark relative returns | Active return/tracking error/information ratio theo cùng windows/units; benchmark constant. |
| A23 | feat(analytics): compute alpha and beta estimates | Regression/risk-free/time aggregation policy, insufficient samples và zero benchmark variance. |
| A24 | test(analytics): verify aligned benchmark statistics | Known linear returns, partial overlap/missing sessions/currency mismatch. |
| A25 | feat(comparison): compare versioned strategy metrics | Baseline/variant deltas kèm interval/frequency/policy compatibility; incompatible metric có lý do. |
| A26 | feat(analytics): serialize reproducible metric reports | Report schema/source digests/calculation version, policy, no overwrite và roundtrip. |
| A27 | feat(analytics): export NAV and drawdown charts | Công cụ đã chọn ở A05, labels/units/gaps và source links; không buộc hosted service. |
| A28 | feat(analytics): render an offline strategy tearsheet | Local HTML hoặc đầu ra A05; escape tên/metadata, không leak private payload hay tải remote scripts ngầm. |
| A29 | test(analytics): verify comparison and report exports | Typed metric → saved report → offline render; unavailable metrics, escaping và file overwrite. |

### Phase A4 — SDK/CLI và nghiệm thu

Phụ thuộc: A2, thêm A3 nhánh được chọn. Cổng: người dùng từ clean install tạo được report từ sealed run và tái tính cùng kết quả; performance chỉ báo từ số đo.

| ID | Commit dự kiến | Đầu ra / kiểm chứng |
| --- | --- | --- |
| A30 | feat(sdk): expose typed strategy analytics workflows | Public calculate/load/report interface và errors; phân biệt partial/complete source. |
| A31 | feat(cli): export strategy analytics from verified evidence | Chỉ commands đã thiết kế, explicit paths/source IDs/output ownership; không giả research route tồn tại. |
| A32 | test(analytics): verify installed analytics workflow | SDK/installed CLI từ external cwd với source evidence, offline replay và comparison. |
| A33 | perf(analytics): measure report generation workloads | Runtime/peak memory/output size cho series length A01 và portfolios; không hardcode latency claim. |
| A34 | docs(analytics): publish metric formulas and acceptance evidence | Formulas/golden examples, interpretation, sample/benchmark limits và reproducible report walkthrough. |
