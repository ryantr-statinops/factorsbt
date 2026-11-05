# Stage 11 — Episode RL với action định trước

**Status:** Accepted; implementation pending

**Phase:** D — Nghiên cứu mở rộng

## Mục tiêu

Kiểm tra một giao diện episode có giới hạn cho policy đã định trước hoặc action đã ghi. SDK gửi trọn chuỗi action trước khi episode chạy; backend trả transition trace sau khi hoàn tất. Đây chưa phải môi trường huấn luyện tương tác tổng quát.

## Hợp đồng nghiên cứu đã chốt

- Fixture có một spot và ba mốc close. Một episode nhận đúng hai action target weights; mỗi action thuộc `{0%, 50%, 100%}`. Sai số lượng hoặc action ngoài tập bị từ chối trước khi enqueue.
- Mỗi job tạo portfolio, cash, positions, pending orders và trace riêng. Action không sửa state trực tiếp mà đi qua admission và lịch thực thi hiện tại.
- Observation tại close ghi simulation time, mark, cash, quantity và NAV. Reward cho action là `NAV(close tiếp theo) − NAV(close hiện tại)`, đơn vị USD, sau khi phí được ghi vào ledger.
- Trace của mỗi bước gồm observation đầu/cuối, action, admission, fills và reward. Reward chỉ có cho bước đã hoàn tất.
- Close cuối dùng finalization hiện tại, giữ vị thế mở và ghi lệnh chờ bị hủy. Nếu lỗi giữa bước, job trả lỗi cùng trạng thái hợp lệ cuối.
- Cung cấp chuỗi luôn giữ tiền mặt và chuỗi giao dịch cố định. Fixture đánh giá tách biệt, có phí để đối chiếu reward bằng phép tính độc lập.
- Episode replay dùng action đã niêm phong; không gọi policy/model mới và không chia sẻ state của episode nguồn.

## Phụ thuộc và giới hạn

- Chia sẻ bước mô phỏng, admission, execution và finalization với engine. SDK chỉ đóng gói request và đọc trace.
- Không chọn thư viện RL, không huấn luyện policy, không hỗ trợ step online, cancellation hoặc random seed protocol.
- Cùng input/action trong phạm vi cùng executable digest và contract; không tuyên bố tái lập xuyên phiên bản binary.

## Task → Commit

| Thứ tự | Task và phạm vi commit | Commit dự kiến |
| --- | --- | --- |
| 1 | Chốt protocol action, observation, reward và episode boundary | `docs(research): define fixed-action episode protocol` |
| 2 | Khởi tạo state episode độc lập và validate toàn bộ chuỗi action | `feat(rl): initialize isolated episode state` |
| 3 | Áp action qua admission và lịch engine; ghi observation/reward/finalization | `feat(rl): apply recorded actions and report rewards` |
| 4 | Nghiệm thu reward có phí, trạng thái cuối và isolation | `test(rl): verify episode rewards and isolation` |

## Điều kiện hoàn thành

- Hai action chạy trên fixture đánh giá và reward khớp phép tính độc lập từ ledger có phí.
- Chuỗi luôn giữ tiền mặt cho reward bằng không khi giá không đổi; chuỗi giao dịch cho trace đủ để giải thích reward.
- Hai episode độc lập không dùng chung portfolio hoặc pending order.
- Lỗi trả trạng thái hợp lệ cuối và không tạo reward cho bước chưa hoàn tất.
