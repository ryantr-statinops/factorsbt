# Stage 11 — Giao diện nghiên cứu reinforcement learning

**Status:** Proposed

**Phase:** D — Nghiên cứu mở rộng

## Mục tiêu

Định nghĩa rồi kiểm chứng một giao diện RL có giới hạn trên simulator, giữ nguyên tính đúng theo thời gian và quy tắc danh mục.

## Phạm vi và đầu ra

- Định nghĩa observation, action, reward, bước môi trường, episode boundary và reset cho một bài toán cụ thể.
- Mapping action qua decision/admission/order/fill hiện có; agent không được cập nhật trực tiếp portfolio để bỏ qua constraints.
- Cách ly episode, scenario, random state và model state; ghi cấu hình, input/action và transition để giải thích kết quả.
- Cung cấp ví dụ bounded cùng baseline và evaluation protocol; không mở rộng thành nền tảng huấn luyện tổng quát.
- Cập nhật interface, adapter và evidence cần thiết sau khi thiết kế được chấp nhận; chưa chọn thư viện RL hoặc public method/schema.

## Phụ thuộc

- [Stage 08](08-scenario-comparison.md), [Stage 09](09-recorded-replay.md): thí nghiệm độc lập và provenance/replay đã được kiểm chứng.
- Không phụ thuộc Stage 10 reranking; không phải điều kiện hoàn thành MVP.

## Quyết định cần chốt — Open

- Bài toán, observation space, action space và mapping sang contract quyết định của engine.
- Reward formula, thời điểm tính, đơn vị, transaction costs, terminal condition, pending orders và reset semantics.
- Train/evaluation split, lịch sử quan sát được phép dùng, seeds và phạm vi kiểm soát state của agent/model ngoài.
- Library/protocol nếu cần, ai điều khiển step, cách biểu diễn lỗi và giới hạn reproducibility.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Chốt RL contract, objective và evaluation protocol | `docs(research): define bounded RL interaction contract` |
| 2 | Triển khai observation/action, episode/reset và reward đã chốt | `feat(rl): connect isolated episodes to the simulation engine` |
| 3 | Ví dụ, baseline và evidence kiểm chứng môi trường | `feat(research): demonstrate verified RL interaction` |

Chỉ bắt đầu task triển khai khi các lựa chọn cần thiết được Accepted; không coi seed hoặc API môi trường là bằng chứng huấn luyện tái lập.

## Kiểm chứng dự kiến

- Observation không chứa dữ liệu chưa khả dụng; action không vượt qua admission/execution policy của engine.
- Tính tay reward trên chuỗi fill controlled, kể cả phí, terminal state và lệnh chờ theo convention đã chốt.
- Reset không giữ lại portfolio/history/model state từ episode trước ngoài phần được cho phép rõ ràng.
- Recorded actions/responses tái hiện transition trong scope đã xác minh; gọi agent/model mới được phân biệt ở provenance.
- Evaluation tôn trọng split và báo cáo giới hạn; không mặc định một vòng tương tác sẽ cải thiện model.

## Điều kiện hoàn thành

- Một hợp đồng RL được chấp nhận, một ví dụ bounded chạy được và evidence cho observation/action/reward/reset.
- Tương tác với vốn, execution và replay đã được kiểm tra; docs ghi rõ phần không hỗ trợ và giới hạn nghiên cứu.
- Hoàn thành RL không ngụ ý reranking đã hoàn thành; hai hướng được theo dõi riêng trong Phase D.

## Tài liệu liên quan

- [Simulation lifecycle](../../docs/architecture/simulation-lifecycle.md)
- [Roadmap — research](../../docs/roadmap.md#stage-3-reranking-and-reinforcement-learning-research)
