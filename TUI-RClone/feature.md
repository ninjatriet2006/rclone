# Bảng Đối Chiếu Tính Năng Rclone Core & TUI-RClone (Rust)

Tệp này được thiết lập để bao quát toàn bộ kiến trúc tính năng và sự tương tác giữa Rclone Core (Go) và giao diện TUI (`TUI-RClone` viết bằng Rust). Khi cần quét hoặc kiểm tra trong các lần sau, hãy đọc tệp này để tránh quét lại toàn bộ mã nguồn gây tốn token.

---

## 1. Kiến Trúc Tương Tác (Integration Architecture)
* **Phương thức liên kết**: `TUI-RClone` liên kết tĩnh với thư viện Core Go thông qua `librclone.a` (nằm ở thư mục gốc `/home/bimatkeo/Documents/Rclone_Clone`).
* **Kênh giao tiếp**: Mã nguồn Rust gọi FFI qua hàm `RcloneRPC(method, input_json)` trong [rclone.rs](file:///home/bimatkeo/Documents/Rclone_Clone/rclone/TUI-RClone/src/rclone.rs).
* **Kết quả**: Tất cả các cải tiến, sửa lỗi ở tầng Backend và Giao thức Core Go của Rclone đều **tự động được áp dụng** cho `TUI-RClone` mà không cần sửa đổi mã Rust.

---

## 2. Bản đồ Tính năng & Tình trạng tích hợp (Feature Map)

| Tính năng Rclone Core | Trạng thái trong TUI-RClone | Phương thức tích hợp / Chi tiết |
| :--- | :--- | :--- |
| **Sửa lỗi Backend** (Google Drive, iCloud, Drime, Proton Drive, S3, SFTP, v.v.) | **Tự động áp dụng** | Thừa hưởng trực tiếp từ Go engine của `librclone.a`. |
| **Cấu hình tùy chọn mới** (ví dụ: `--sftp-encoding`) | **Tự động áp dụng** | TUI truy vấn động qua RPC `config/providers`. Các tham số mới tự động hiển thị trong giao diện Advanced Setup của TUI. |
| **SFTP/NFS Server** (`serve sftp`, `serve nfs`) | **Tự động áp dụng** | TUI quản lý thông qua việc thực thi lệnh hệ thống `rclone serve <proto>`. Mọi cập nhật tính năng (như `statvfs`, mount subpath) tự động chạy qua binary `rclone`. |
| **OAuth Authorization URL** (`config/oauthstatus`) | **Tích hợp thủ công** | Đã được tích hợp trong Rust. TUI chạy luồng polling định kỳ truy vấn `config/oauthstatus` để cập nhật và hiển thị URL xác thực thực tế cho người dùng. |
| **Hủy OAuth Server** (`config/oauthstop`) | **Tích hợp thủ công** | Đã tích hợp. Khi người dùng nhấn phím `ESC` để thoát luồng Simple OAuth, TUI gửi lệnh gọi RPC `config/oauthstop` để tắt web server Go ngầm một cách sạch sẽ. |

---

## 3. Các tính năng tùy biến độc lập của TUI-RClone (Rust-only)
Các tính năng sau đây được phát triển hoàn toàn ở tầng Rust UI của `TUI-RClone`:
1. **Multi-select (Chọn nhiều tệp tin)**: Hỗ trợ phím tắt `Ctrl+A`, range selection qua `Shift+Space`/`Alt+Space` và clipboard xử lý hàng loạt (`Ctrl+C`, `Ctrl+X`, `Ctrl+V`, `Delete`).
2. **Dynamic Progress (Cập nhật tiến độ động)**: Hàm `run_rpc_job_async` truy vấn `core/stats` để lấy phần trăm tải lên/tải xuống theo thời gian thực và đẩy qua channel.
3. **Thanh tiến trình đồ họa (ProgressBar)**: Hiển thị trạng thái các tiến trình đang chạy trực quan dạng `[████████░░░░]` trong tab Job Monitor.
4. **Cơ chế Throttling (Giảm tải)**: Khống chế chu kỳ quét dịch vụ nền (mỗi 4s) và stats (mỗi 1.5s) để chống lag/crash TUI.

---

## 4. Lịch sử cập nhật đồng bộ (Sync Changelog)
* **2026-05-31**: Đồng bộ các cập nhật của nhánh `upstream/master` (sau bản phát hành `v1.74.2`). Tích hợp `config/oauthstatus` hiển thị URL thực tế và `config/oauthstop` giải phóng tài nguyên khi hủy OAuth.
