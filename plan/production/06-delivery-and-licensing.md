# 6. Triển khai, CI/CD và phân phối

**Status:** Proposed. Môi trường deploy, ma trận hỗ trợ, license và chính sách phát hành còn Open.

## Hiện trạng đã rà soát

- Repo chưa có Dockerfile/Compose hoặc workflow CI GitHub Actions được tìm thấy trong cây làm việc.
- `python-sdk/pyproject.toml` đã cấu hình package build bằng setuptools và console script; điều này chưa chứng minh package đã phát hành lên PyPI.
- Repo chưa có `LICENSE`; tài liệu xác nhận license chưa được chọn. Chưa có tuyên bố hỗ trợ production deployment hay public API stability.

Tham chiếu: [pyproject.toml](../../python-sdk/pyproject.toml), [license/distribution status](../../docs/README.md), [deployment boundaries](../../docs/architecture/overview.md).

## Câu hỏi cần quyết định

- Người dùng mục tiêu cần local installation, single-server, team deployment hay Kubernetes? Linux/macOS/Windows nào cần hỗ trợ?
- Backend, fixture/data volume, persistent store và secrets được cấu hình thế nào trong môi trường đích?
- CI cần kiểm tra lint/format, unit/contract tests, package build, integration smoke và trên hệ điều hành/toolchain nào?
- Chọn license sau khi rà quyền sở hữu code và dependency; license choice cần được chấp thuận trước public distribution.
- Release versioning, wheel compatibility, source distribution, publishing credentials và support policy ra sao?

Docker, GitHub Actions và các license MIT/Apache-2.0 là các mục cần quyết định, chưa phải lựa chọn được chấp thuận. Không đưa credential thật vào repo hay log.

## Tiêu chí cho lát cắt triển khai

- Hướng dẫn build/deploy tái lập được trong môi trường mục tiêu đã nêu.
- CI chạy đúng checks và matrix đã thống nhất; kết quả verification được ghi cùng môi trường.
- License được chọn và đặt đúng trước public distribution; dependency/license obligations được rà soát.
- Package artifact được build và kiểm tra cài đặt theo chính sách phát hành; chỉ tuyên bố PyPI release sau khi thực sự phát hành.
