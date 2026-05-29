---
name: self-evolution
description: Quy chuẩn và giao thức tự cập nhật, nâng cấp và cải tiến các tệp tài liệu kỹ năng (skill) của dự án sau khi sửa lỗi hoặc hoàn thành tính năng thành công.
user-invocable: false
---

# Quy Chuẩn Tự Tiến Hóa & Cập Nhật Skill (Self-Evolution Protocol)

Tài liệu này định nghĩa giao thức hoạt động giúp các trợ lý AI (Agent) tự động cập nhật và nâng cấp tri thức của dự án qua các phiên làm việc khác nhau.

---

## 1. Nguyên Tắc Cốt Lõi
Vì bộ nhớ của Agent sẽ bị xóa sạch sau mỗi cuộc hội thoại (conversation), mọi bài học kinh nghiệm, quy chuẩn sửa lỗi và thiết kế hệ thống quan trọng bắt buộc phải được lưu trữ dưới dạng tệp tin `.md` (skill files) trong repository này để các Agent thế hệ tiếp theo kế thừa.

## 2. Quy Trình Tự Cập Nhật Của Agent
Mỗi khi Agent hoàn thành một trong hai tác vụ sau:
1. **Sửa lỗi thành công:** Khắc phục xong một lỗi biên dịch, runtime, cấu hình CI/CD hoặc lỗi logic ứng dụng.
2. **Thêm tính năng mới thành công:** Thiết kế và tích hợp xong một component, module hoặc tính năng mới.

**Agent bắt buộc phải:**
* Xác định tệp skill liên quan trong dự án (ví dụ: `github-free-tier-solve-skill.md`, `rust-tui-development-skill.md`).
* Chủ động cập nhật chi tiết: **Nguyên nhân lỗi/Yêu cầu thiết kế** và **Giải pháp khắc phục/Cách hiện thực chuẩn** trực tiếp vào tệp skill đó trước khi kết thúc phiên làm việc.
* Đảm bảo cấu trúc tài liệu rõ ràng, dễ hiểu để các Agent sau có thể đọc và áp dụng ngay lập tức mà không cần hỏi lại người dùng.
