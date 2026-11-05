# Stage 12 — CLI backtest và recorded replay

**Status:** Implemented — Phase E CLI đã qua cổng local

**Phase:** E — Brainstorm ứng dụng, chưa chốt

## Phạm vi Phase E đã chốt

Tạo lệnh `factorsbt` cài vào virtualenv bằng `python -m pip install ./python-sdk`. CLI không tương tác dùng Python SDK hiện có để gửi backtest, theo dõi run, đọc kết quả và kiểm chứng recorded replay.

Không thuộc phạm vi: console tương tác, comparison, tải artifact, research jobs và phát hành PyPI.

## Lệnh

| Lệnh | Hành vi |
| --- | --- |
| `factorsbt health` | Kiểm tra backend và contract version. |
| `factorsbt run submit CONFIG.json [--submission-id UUID]` | Đọc Run Configuration v1 UTF-8 và gửi một lần. |
| `factorsbt run lookup SUBMISSION_ID` | Tra cứu submission, gồm trường hợp mất acknowledgment. |
| `factorsbt run status RUN_ID` / `wait RUN_ID` / `result RUN_ID` | Xem lifecycle, chờ kết thúc hoặc lấy kết quả/evidence. |
| `factorsbt replay submit SOURCE_RUN_ID [--replay-id UUID]` | Tạo recorded replay từ run nguồn. |
| `factorsbt replay status REPLAY_ID` / `wait REPLAY_ID` / `report REPLAY_ID` | Xem lifecycle, chờ hoặc lấy report kiểm chứng. |

## Cấu hình và đầu ra

- Endpoint theo thứ tự `--base-url`, `FACTORSBT_BASE_URL`, `http://127.0.0.1:8080`; chỉ cho phép HTTP loopback `localhost`, `127.0.0.1`, `[::1]` với port tùy chọn.
- SDK request timeout mặc định 5 giây. `wait` nhận timeout local dương, hữu hạn (mặc định 60 giây) và poll mỗi 0,25 giây; hết timeout không hủy job.
- Lệnh thành công in một JSON object ra stdout; decimal là chuỗi, timestamp UTC và evidence là mảng. Help là văn bản.
- Lỗi in JSON có mã lỗi và ID liên quan ra stderr, stdout rỗng. Exit: 0 thành công/đọc được status hoặc report; 1 HTTP/transport/protocol; 2 tham số hoặc JSON đầu vào; 3 job thất bại khi wait; 4 timeout; 5 acknowledgment không rõ; 130 Ctrl+C.
- Khi submit mất acknowledgment, in ID và lệnh lookup/status cần dùng; không tự gửi lại POST. Replay giữ cùng executable digest và contract version theo Phase C.

## Đóng gói

`python-sdk` được cài bằng setuptools theo src layout, package `0.1.0`, Python `>=3.13`, không thêm dependency runtime; entry point là `factorsbt = "factorsbt.cli:main"`.

## Task → Commit

1. `docs(plan): define Phase E CLI scope` — chốt lệnh, phạm vi và nghiệm thu.
2. `feat(cli): add command entry point and local settings` — parser, cấu hình endpoint, help và health.
3. `build(python): package the FactorsBT console command` — metadata và entry point cài đặt.
4. `feat(cli): submit and look up backtest runs` — đọc config, submit và tra cứu submission.
5. `feat(cli): inspect wait for and retrieve run results` — status, wait và result.
6. `feat(cli): submit and verify recorded replays` — thao tác replay đầy đủ.
7. `fix(cli): standardize command failures and exit codes` — stdout/stderr, lỗi và mã thoát.
8. `docs(cli): document installation and terminal workflows` — setup và ví dụ.
9. `test(cli): verify installed backtest and replay workflows` — test cuối phase và smoke qua lệnh đã cài.
10. `docs(plan): close Phase E verification` — bằng chứng, tiến độ và graph.

## Kiểm chứng và điều kiện hoàn thành

- Chạy mọi unittest ở ranh giới nghiệm thu cuối Phase E, cùng kiểm tra argument, JSON sai, độ chính xác decimal, lỗi backend, timeout, Ctrl+C và acknowledgment mơ hồ; xác nhận POST không retry.
- Cài non-editable vào virtualenv sạch, chạy từ ngoài repo với `PYTHONPATH` rỗng: health → submit → lookup → wait → result → replay submit/wait/report sau khi model fixture dừng; report phải matched.
- Chạy các smoke backtest/failure hiện có và kiểm tra package/import. Chỉ ghi `Implemented` khi các cổng pass.
- Làm trên `main`; sau nghiệm thu cập nhật Phase E 1/1 và roadmap 13/13, kiểm tra fast-forward rồi push `origin/main`. CI chưa có theo bằng chứng hiện tại.

## Bằng chứng hoàn thành — 2026-11-05

- Package được cài non-editable trong virtualenv sạch; lệnh đã cài chạy từ ngoài repository với `PYTHONPATH` rỗng qua health, run submit/lookup/wait/result và recorded replay. Replay matched khi model fixture đã dừng.
- 68 Python tests, smoke CLI, hai smoke SDK/backend và Rust fmt/check/strict Clippy/154 workspace tests pass.
- Chi tiết lệnh, môi trường, kết quả và giới hạn được ghi ở [Phase E local verification](../../docs/development/phase-e-local.md).

## Tài liệu liên quan

- [Product overview](../../docs/product/overview.md)
- [Architecture overview](../../docs/architecture/overview.md)
- [Python SDK](../../python-sdk/src/factorsbt/client.py)
- [Phase C verification](../../docs/development/phase-c-local.md)
