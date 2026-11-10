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
