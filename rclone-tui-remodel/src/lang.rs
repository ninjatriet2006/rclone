use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs;
use std::sync::RwLock;

lazy_static! {
    static ref TRANSLATIONS: RwLock<HashMap<String, String>> = RwLock::new(HashMap::new());
}

/// Khởi tạo thư mục và tệp ngôn ngữ mặc định nếu chưa tồn tại
pub fn init_languages() {
    let lang_dir = crate::app_config::AppConfig::config_dir().join("lang");
    if !lang_dir.exists() {
        let _ = fs::create_dir_all(&lang_dir);
    }
    let default_vn = r#"# Rclone-TUI Vietnamese Translations
unikey_tip: "💡 Mẹo: Nếu Unikey tự chuyển dấu sai (ví dụ Telex), hãy tắt bộ gõ tiếng Việt (chuyển sang chữ E) trong hệ điều hành."
remote: "Remote nguồn cần mã hóa (ví dụ: Telebox:ThuMucGoc)"
remote_friendly: "Remote nguồn"
filename_encryption: "Cách mã hóa tên file: standard (mặc định), off (tắt), base32"
filename_encryption_friendly: "Mã hóa tên file"
directory_name_encryption: "Mã hóa tên thư mục: true (bật) hoặc false (tắt)"
directory_name_encryption_friendly: "Mã hóa tên thư mục"
password: "Mật khẩu dùng để mã hóa dữ liệu của bạn"
password_friendly: "Mật khẩu"
password2: "Mật khẩu Salt bổ sung (tùy chọn nhưng khuyến nghị, nên khác mật khẩu chính)"
password2_friendly: "Mật khẩu Salt phụ"
client_id: "OAuth Client ID của ứng dụng (để trống nếu dùng mặc định)"
client_id_friendly: "Client ID"
client_secret: "OAuth Client Secret của ứng dụng (để trống nếu dùng mặc định)"
client_secret_friendly: "Client Secret"
description: "Mô tả ngắn gọn cho kết nối này"
description_friendly: "Mô tả"
token: "OAuth access token (thường được tạo tự động)"
token_friendly: "Access Token"
filename_encoding: "Cách mã hóa tên file đã mã hóa thành chuỗi văn bản (rút ngắn tên file)."
filename_encoding_friendly: "Mã hóa chuỗi tên"
no_data_encryption: "Tùy chọn mã hóa dữ liệu file hoặc để nguyên không mã hóa."
no_data_encryption_friendly: "Mã hóa dữ liệu file"
pass_bad_blocks: "Nếu bật, các block bị lỗi (bad blocks) sẽ được bỏ qua dưới dạng dữ liệu toàn số 0."
pass_bad_blocks_friendly: "Bỏ qua block lỗi"
server_side_across_configs: "Cho phép các thao tác trực tiếp trên server (ví dụ: copy) hoạt động giữa các cấu hình khác nhau."
server_side_across_configs_friendly: "Copy xuyên Server"
show_mapping: "Hiển thị cách ánh xạ mã hóa tên của các tệp tin trong log."
show_mapping_friendly: "Hiển thị ánh xạ"
strict_names: "Nếu bật, sẽ báo lỗi nếu gặp file không thể giải mã tên (mặc định rclone chỉ cảnh báo)."
strict_names_friendly: "Bắt buộc giải mã tên"
suffix: "Hậu tố phần mở rộng cho các file mã hóa (mặc định là .bin)."
suffix_friendly: "Hậu tố mở rộng"
upstreams: "Danh sách các remote thành phần (upstreams), phân tách bằng khoảng trắng. Ví dụ: 'remote1:test/dir remote2:', '\"remote1:test/space:ro dir\" remote2:', v.v."
upstreams_friendly: "Upstreams"
action_policy: "Chính sách lựa chọn remote thành phần cho các thao tác ACTION."
action_policy_friendly: "Chính sách ACTION"
create_policy: "Chính sách lựa chọn remote thành phần cho các thao tác CREATE."
create_policy_friendly: "Chính sách CREATE"
search_policy: "Chính sách lựa chọn remote thành phần cho các thao tác SEARCH."
search_policy_friendly: "Chính sách SEARCH"
cache_time: "Thời gian lưu tạm thông tin sử dụng và dung lượng trống (tính bằng giây). Chỉ có tác dụng khi dùng chính sách bảo toàn đường dẫn (path preserving)."
cache_time_friendly: "Thời gian cache"
min_free_space: "Dung lượng trống tối thiểu cần thiết cho các chính sách lfs/eplfs. Nếu một remote có ít hơn dung lượng này, nó sẽ không được xem xét."
min_free_space_friendly: "Dung lượng trống tối thiểu"
tip_select_remote: "💡 Sử dụng Mũi tên Lên/Xuống hoặc Trái/Phải để chọn Remote có sẵn"
tip_select_choice: "💡 Sử dụng Mũi tên Lên/Xuống hoặc Trái/Phải để chọn giá trị"
help_editing: "[Enter] Hoàn tất nhập | [ESC] Hủy nhập"
help_navigation: "[Up/Down]Di chuyển|[Enter]Chọn/Sửa|[Backspace]Xóa tham số|[ESC]Hủy & Quay lại"
help_general: "[Mũi tên/Tab]Di chuyển|[Enter]Chọn|[Space]Chọn checkbox|[ESC]Hủy & Quay lại"
status_online: "🟢 Trực tuyến"
status_online_loading: "🟢 Trực tuyến (Đang tính dung lượng...)"
status_online_unlimited: "🟢 Trực tuyến (Không giới hạn)"
status_unchecked: "Chưa kiểm tra"
confirm_delete_remote: "Bạn có chắc chắn muốn xóa cấu hình remote '{}'?\n\n[Enter / Y] Xác nhận xóa\n[Esc / N] Hủy bỏ"
confirm_delete_remote_title: " XÁC NHẬN XÓA REMOTE "
confirm_delete_file: "Bạn có chắc chắn muốn xóa tệp/thư mục '{}'?\nLưu ý: Thư mục sẽ bị xóa vĩnh viễn cùng tất cả nội dung bên trong!\n\n[Enter / Y] Xác nhận xóa | [Esc / N] Hủy bỏ"
confirm_delete_file_title: " XÁC NHẬN XÓA MỤC "
confirm_delete_multiple: "Bạn có chắc chắn muốn xóa {} mục đã chọn?\nLưu ý: Tất cả các mục này cùng toàn bộ nội dung bên trong sẽ bị xóa vĩnh viễn!\n\n[Enter / Y] Xác nhận xóa | [Esc / N] Hủy bỏ"
confirm_delete_multiple_title: " XÁC NHẬN XÓA NHIỀU MỤC "
confirm_delete_service: "Bạn có chắc chắn muốn dừng và gỡ bỏ dịch vụ ngầm:\n'{}'?\n\n[Enter / Y] Xác nhận tắt | [Esc / N] Hủy bỏ"
confirm_delete_service_title: " XÁC NHẬN TẮT DỊCH VỤ NGẦM "
confirm_delete_systemd: "Bạn có chắc chắn muốn dừng, tắt và XÓA VĨNH VIỄN dịch vụ hệ thống:\n'{}'?\nThao tác này sẽ gỡ cấu hình dịch vụ khỏi đĩa cứng!\n\n[Enter / Y] Xác nhận xóa | [Esc / N] Hủy bỏ"
confirm_delete_systemd_title: " XÁC NHẬN XÓA DỊCH VỤ SYSTEMD "

conn_wizard_auth_headless: "3. Headless OAuth (Xác thực thủ công khi chạy trên máy chủ không có giao diện/trình duyệt)"
conn_wizard_headless_title: " XÁC THỰC HEADLESS OAUTH ({}) "
conn_wizard_headless_prompt: "Hãy chạy lệnh sau trên máy tính cá nhân của bạn (có trình duyệt web):\n\n  rclone authorize \"{}\"\n\nSau đó copy chuỗi JSON token nhận được và dán vào ô dưới đây rồi nhấn Enter để xác nhận:"
exp_special_title: " HỘP THOẠI CHỨC NĂNG ĐẶC BIỆT "
exp_special_link: "1. Lấy link tải công khai (operations/publiclink)"
exp_special_hash: "2. Tính mã Hash / Checksum (operations/hashsumfile)"
exp_special_cleanup: "3. Dọn dẹp rác Cloud (operations/cleanup)"
exp_special_rmdir: "4. Xóa an toàn thư mục rỗng (rmdir)"
exp_special_rmdirs: "5. Xóa đệ quy thư mục rỗng (rmdirs)"
exp_special_cryptdecode: "6. Giải mã tên tệp mã hóa (cryptdecode)"
exp_special_archive: "7. Giải nén Archive (archive)"
exp_special_dedupe: "8. Lọc trùng tệp/thư mục (dedupe)"
exp_special_merge_similar: "9. Gộp thư mục tương tự (Cách/Viết hoa)"
exp_special_close: "[Esc] Đóng Menu"
exp_merge_similar_title: " GỘP THƯ MỤC TƯƠNG TỰ "
exp_merge_similar_prompt: "Phát hiện {} nhóm thư mục tương tự nhau sẽ được gộp:\n\n{}"
exp_merge_similar_help: "[Enter] Xác nhận gộp | [Esc] Hủy"
exp_no_similar_dirs: "Không tìm thấy thư mục tương tự nào (khác nhau về khoảng trắng hoặc viết hoa/thường)!"
exp_dedupe_title: " LỌC TRÙNG TỆP / THƯ MỤC (DEDUPE) "
exp_dedupe_prompt: "Chọn chế độ giải quyết tệp tin trùng lặp:"
exp_dedupe_mode_rename: "1. Đổi tên tất cả các tệp trùng để giữ lại hết (Rename)"
exp_dedupe_mode_newest: "2. Giữ tệp mới nhất, xóa tệp cũ hơn (Keep newest)"
exp_dedupe_mode_oldest: "3. Giữ tệp cũ nhất, xóa tệp mới hơn (Keep oldest)"
exp_dedupe_mode_largest: "4. Giữ tệp lớn nhất, xóa các tệp nhỏ hơn (Keep largest)"
exp_dedupe_mode_smallest: "5. Giữ tệp nhỏ nhất, xóa các tệp lớn hơn (Keep smallest)"
exp_dedupe_mode_first: "6. Giữ tệp đầu tiên tìm thấy (Keep first)"
exp_dedupe_mode_skip: "7. Bỏ qua tất cả các tệp trùng (Skip)"
exp_dedupe_by_hash_prompt: "Bạn muốn tìm trùng lặp theo cách nào?\n[Space] Bật/Tắt: Tìm theo Hash thay vì theo Tên file\nTrạng thái: {}"
exp_dedupe_help: "[Up/Down] Chọn chế độ | [Space] Bật/Tắt tìm theo Hash | [Enter] Bắt đầu | [Esc] Hủy"
exp_copy_mode_title: " CHỌN CHẾ ĐỘ TRUYỀN TẢI "
exp_copy_mode_prompt: "Chọn cách rclone đối chiếu tệp tin nguồn và đích:"
exp_copy_mode_normal: "1. Sao chép thông thường (Dựa trên kích thước & thời gian sửa đổi)"
exp_copy_mode_checksum: "2. Đối chiếu mã Hash (Check hash đệ quy, trùng hash bỏ qua, khác hash copy đè)"
exp_copy_mode_help: "[Up/Down] Chọn chế độ | [Enter] Xác nhận | [Esc] Hủy"
exp_hash_title: " CHỌN LOẠI MÃ CHECKSUM "
exp_hash_prompt: "Dùng phím Mũi tên và Enter để chọn loại checksum:"
exp_cryptdecode_title: " GIẢI MÃ TÊN FILE MÃ HÓA (CRYPTDECODE) "
exp_cryptdecode_prompt_remote: "1. Tên Remote Crypt (ví dụ: mycrypt:):"
exp_cryptdecode_prompt_name: "2. Tên/Đường dẫn mã hóa cần giải mã:"
exp_cryptdecode_help: "[Tab] Chuyển trường | [Enter] Bắt đầu giải mã | [Esc] Hủy"
exp_archive_title: " CHỌN CHẾ ĐỘ GIẢI NÉN "
exp_archive_here: "1. Extract tại đây (Extract Here)"
exp_archive_folder: "2. Thư mục riêng (Extract to Folder)"
exp_archive_path: "3. Vị trí chỉ định (Extract to Path)"
exp_archive_path_title: " PHƯƠNG THỨC CHỌN ĐƯỜNG DẪN ĐÍCH "
exp_archive_path_manual: "1. Nhập tay đường dẫn (Terminal-style)"
exp_archive_path_tui: "2. Duyệt chọn vị trí (TUI Explorer-style)"
exp_archive_manual_prompt: "Nhập đường dẫn đích giải nén:"
exp_archive_tui_prompt: "Duyệt qua các remote/thư mục và nhấn [Insert] tại thư mục đích muốn giải nén vào:"
exp_archive_tui_help: "[Up/Down]Di chuyển|[Enter]Vào thư mục|[Insert]Chọn|[Esc]Thoát"
exp_paste_rename_title: " SAO CHÉP VÀ ĐỔI TÊN (COPYTO) "
exp_paste_rename_prompt: "Nhập tên mới cho tệp/thư mục trước khi dán:"

# Main Menu
menu_1: "1. Connection Manager (Quản lý kết nối Cloud)"
menu_2: "2. File Explorer (Trình duyệt tệp tin đám mây)"
menu_3: "3. Job Monitor (Giám sát tác vụ đồng bộ)"
menu_4: "4. Config Profile Manager (Hồ sơ cấu hình)"
menu_5: "5. Services & Utilities (Ổ ảo FUSE, Web GUI, Serve)"
menu_6: "6. Language Settings (Cài đặt ngôn ngữ)"
menu_install_dep: "7. Install Dependencies (Cài đặt phụ thuộc: FUSE, Filen CLI)"
menu_7: "8. Exit (Thoát ứng dụng)"
menu_welcome: "Chào mừng đến với Rclone Clone TUI! Sử dụng phím Mũi tên Lên/Xuống để điều hướng và Enter để chọn."
menu_title: " MENU CHÍNH "
conn_insert_api_key_hint: " [Insert: Nhập tự động API Key]"
conn_insert_api_key_missing_hint: " [Insert: Yêu cầu cài đặt filen-cli]"

# Connection Manager
conn_help_navigation: "[Insert]Thêm mới|[Alt+E]Chỉnh sửa|[Delete]Xóa kết nối|[?]Tính năng|[Mũi tên]Di chuyển|[ESC]Về Menu chính"
conn_title: " CLOUD REMOTES (DANH SÁCH KẾT NỐI) "
conn_error_title: " LỖI "
conn_info_title: " THÔNG BÁO "
conn_wizard_provider_title: " BƯỚC 1: CHỌN CLOUD PROVIDER (NHẤN SPACE ĐỂ CHỌN NHIỀU) "
conn_wizard_provider_configuring: "Đang cấu hình nhà cung cấp: "
conn_wizard_name_prompt: "Nhập tên kết nối (Remote Name):"
conn_wizard_name_title: " BƯỚC 2: ĐẶT TÊN KẾT NỐI "
conn_wizard_auth_simple: "1. Simple OAuth (Tự động mở trình duyệt xác thực)"
conn_wizard_auth_advanced: "2. Advanced Configuration (Cấu hình nâng cao - Nhập thủ công Client ID/Secret)"
conn_wizard_auth_mode_title: " BƯỚC 3: CHỌN CHẾ ĐỘ XÁC THỰC ({}) "
conn_wizard_oauth_title: " XÁC THỰC SIMPLE OAUTH "
conn_wizard_oauth_started: "Xác thực Simple OAuth cho remote: "
conn_wizard_oauth_open_browser: "Ứng dụng đã cố gắng mở trình duyệt web của bạn tại địa chỉ URL dưới đây."
conn_wizard_oauth_copy_url: "If browser does not open automatically, copy this URL and paste to browser:"
conn_wizard_oauth_waiting: "ĐANG CHỜ PHẢN HỒI XÁC THỰC TỪ TRÌNH DUYỆT (Nhấn ESC để hủy)..."
conn_wizard_edit_title: " THIẾT LẬP NÂNG CAO CHO REMOTE: {} ({}) "
conn_wizard_edit_title_simple: " CHỈNH SỬA REMOTE: {} ({}) "
conn_wizard_edit_tab_basic: " [1] CƠ BẢN (BASIC) "
conn_wizard_edit_tab_adv: " [2] NÂNG CAO (ADVANCED) "
conn_wizard_edit_tab_help: "     (Dùng phím TAB để chuyển tab)"
conn_wizard_edit_save: "LƯU THAY ĐỔI"
conn_wizard_edit_cancel: "[ESC] QUAY LẠI"
conn_remote_name_label: "Tên Remote (Remote Name)"

# File Explorer
exp_help: "[Tab]Chuyển đổi khung|[Enter/BS]Vào/Lùi|[Alt+R]Remote|[Alt+Y]Đổi tên|[Ctrl+C/V]Sao chép/Dán|[Ctrl+X]Di chuyển|[Delete]Xóa|[Alt+N]Thư mục mới|[Alt+T]Đồng bộ|[Alt+V]Chọn đơn|[Shift+V]Chọn vùng|[Alt+O]Chức năng khác|[ESC]Quay lại"
exp_console_title: " BẢNG TRẠNG THÁI / CONSOLE "
exp_console_empty: "Không có lệnh nào đang chờ (Nhấn Ctrl+C để sao chép mục)"
exp_console_pending: "ĐANG CHỜ SAO CHÉP từ: {} (Nhấn Ctrl+V để dán, Space để hủy)"
exp_copy_title: " TIẾN ĐỘ SAO CHÉP "
exp_copy_msg: "Đang sao chép file/thư mục...\nTừ: {}\nĐến: {}\nTiến độ: {:.1}%\n[Nhấn Ctrl+C để hủy]"
exp_move_title: " TIẾN ĐỘ DI CHUYỂN "
exp_move_msg: "Đang di chuyển file/thư mục...\nTừ: {}\nĐến: {}\nTiến độ: {:.1}%\n[Nhấn Ctrl+C để hủy]"
exp_sync_title: " XÁC NHẬN ĐỒNG BỘ "
exp_sync_msg: "Bạn có chắc chắn muốn Đồng bộ (Sync) dữ liệu một chiều?\n\nNguồn (Active): {}{}\nĐích (Target): {}{}\n\n[LƯU Ý: Thao tác này sẽ xóa các tệp ở ĐÍCH nếu NGUỒN không có!]\n\n[Enter / Y] Tiếp tục | [Esc / N] Hủy"
exp_loading: "  Đang tải dữ liệu..."
exp_empty: "  (Thư mục rỗng)"
exp_creating_placeholder: "Đang tạo..."
exp_new_folder_prompt: "Nhập tên thư mục mới cần tạo:"
exp_new_folder_title: " TẠO THƯ MỤC MỚI "
exp_select_remote_title: " CHỌN KHÔNG GIAN LƯU TRỮ (REMOTE) "
exp_add_shared_link_option: "[Thêm Link Shared (Google Drive)]"
exp_input_shared_link_prompt: "Nhập link shared Google Drive (Thư mục):"
exp_select_base_remote_title: " CHỌN REMOTE XÁC THỰC CỦA BẠN "
exp_select_base_remote_prompt: "Chọn một remote Google Drive để làm gốc xác thực:"
exp_permission_error_title: " CẢNH BÁO QUYỀN TRUY CẬP "
exp_permission_error_prompt: "Phát hiện lỗi không có quyền tải xuống (download restricted) đối với tệp này!"
exp_permission_option_cancel: "Không sao chép (Hủy)"
exp_permission_option_as_much: "Sao chép nhiều nhất có thể (Bỏ qua tệp lỗi, vẫn tạo thư mục)"
exp_permission_option_restricted: "Sao chép hạn chế (Chỉ tạo thư mục chứa tệp tải được)"
exp_error_title: " LỖI EXPLORER "

# Config Profile Manager
prof_title: " QUẢN LÝ HỒ SƠ CẤU HÌNH (PROFILES) "
prof_help: "[Enter]Kích hoạt Profile|[Ctrl+X]Xuất|[Insert]Nhập Profile|[ESC]Về Menu chính"
prof_help_wizard: "[Mũi tên/Tab]Di chuyển|[Enter]Chọn|[ESC]Hủy bỏ"
prof_overwrite_import_msg: "Tên Profile '{}' đã tồn tại trong danh sách.\nBạn có muốn ghi đè đường dẫn tệp cấu hình của nó không?\n\n[Enter] Ghi đè | [ESC] Hủy"
prof_overwrite_import_title: " XÁC NHẬN GHI ĐÈ IMPORT "
prof_overwrite_export_msg: "Tệp cấu hình của Profile '{}' đã tồn tại trong thư mục Downloads/Saved Profile/.\nBạn có muốn ghi đè lên nó không?\n\n[Enter] Ghi đè | [ESC] Hủy"
prof_overwrite_export_title: " XÁC NHẬN GHI ĐÈ XUẤT "
prof_export_success: "Xuất Profile thành công!\nTệp tin được lưu tại:\n\n👉 {}\n\n[Nhấn Enter hoặc ESC để đóng]"
prof_success_title: " THÀNH CÔNG "
prof_error_title: " LỖI CẤU HÌNH "
prof_new_prompt: "Nhập tên Profile mới cần tạo:"
prof_new_title: " BƯỚC 1: TÊN PROFILE "
prof_type_url: "1. Link Direct (Tải từ đường dẫn URL cấu hình trực tiếp)"
prof_type_local: "2. Copy & Pull (Sao chép từ tệp cấu hình local có sẵn)"
prof_type_title: " BƯỚC 2: PHƯƠNG THỨC NHẬP ({}) "
prof_url_prompt: "Nhập đường dẫn URL cấu hình trực tiếp:"
prof_file_prompt: "Nhập đường dẫn tệp cấu hình local (Ví dụ: /home/user/rclone.conf):"
prof_importing: "Đang thêm Profile: {}"
prof_source_title: " BƯỚC 3: ĐƯỜNG DẪN NGUỒN CẤU HÌNH "

# Services
srv_opt_mount: "1. Mount Virtual Drive via FUSE (Tạo ổ đĩa ảo bằng FUSE)"
srv_opt_nfsmount: "2. Mount Virtual Drive via NFS (Tạo ổ đĩa ảo bằng NFS)"
srv_opt_gui: "3. Open Web GUI (Mở giao diện Web quản trị Rclone)"
srv_opt_serve: "4. Quick File Sharing Server (Tạo máy chủ chia sẻ nhanh)"
srv_title_config: " CẤU HÌNH DỊCH VỤ "
srv_title_tui: " DỊCH VỤ CHẠY NGẦM QUA TUI "
srv_tag_user: " [Cá nhân]"
srv_tag_system: " [Hệ thống]"
srv_title_systemd: " DỊCH VỤ HỆ THỐNG SYSTEMD (RCLONE) "
srv_help_t0: "[Tab]Đổi bảng|[Enter]Chọn|[ESC]Quay lại Menu chính"
srv_help_t1: "[Tab]Đổi bảng|[Delete]Tắt dịch vụ ngầm TUI|[ESC]Quay lại Menu chính"
srv_help_t2: "[Tab]Đổi bảng|[Enter]Chỉnh sửa cấu hình|[Space]Thao tác dịch vụ|[Insert]Tạo mới|[Delete]Xóa dịch vụ|[ESC]Quay lại"
srv_help_action: "[Mũi tên]Chọn hành động|[Enter]Thực thi|[ESC]Hủy bỏ"
srv_help_edit: "[Tab/Mũi tên Trái/Phải]Đổi Tab|[Mũi tên Lên/Xuống]Di chuyển trường|[Enter]Sửa|[Delete]Xóa khóa (Tab Nâng cao)|[Insert]Thêm khóa (Tab Nâng cao)|[ESC]Hủy"
srv_help_general: "[Tab/Mũi tên]Di chuyển|[Enter]Chọn|[ESC]HỦY BỎ toàn bộ thiết lập dịch vụ"
srv_local_system: "[Cục bộ / Local System]"
srv_edit_systemd_title: "CHỈNH SỬA DỊCH VỤ SYSTEMD: {}"
srv_new_systemd_title: "TẠO MỚI DỊCH VỤ SYSTEMD (RCLONE MOUNT)"
srv_error_title: " LỖI DỊCH VỤ "
srv_info_title: " THÔNG BÁO "
srv_local_desc: "  [Local] -> Sử dụng bộ nhớ máy tính cục bộ"
srv_cloud_desc: "  [Cloud] -> {}"
srv_select_source_title: " THIẾT LẬP DỊCH VỤ {}: CHỌN NGUỒN DỮ LIỆU "
srv_mount_point_prompt: "Nhập đường dẫn thư mục MOUNT POINT cục bộ (Ví dụ: /home/user/mnt):"
srv_share_path_prompt: "Nhập đường dẫn con bên trong remote muốn chia sẻ (Để trống nếu chia sẻ toàn bộ):"
srv_selected_source: "Nguồn dữ liệu đã chọn: "
srv_mount_point_title: " THIẾT LẬP THƯ MỤC / MOUNT POINT "
srv_proto_http: "1. http (Dễ dùng, xem trực tiếp qua trình duyệt)"
srv_proto_ftp: "2. ftp (Tương thích tốt với các client FTP truyền thống)"
srv_proto_webdav: "3. webdav (Thích hợp mount ổ đĩa mạng trên hệ điều hành khác)"
srv_proto_sftp: "4. sftp (Bảo mật tối đa, mã hóa lưu lượng)"
srv_select_proto_title: " CHỌN GIAO THỨC CHIA SẺ ({}{}) "
srv_config_for: "Cấu hình dịch vụ cho: "
srv_wizard_progress: "Tiến trình cấu hình cờ tùy chọn (Flag): {} / {}"
srv_wizard_flag: "Cờ tùy chọn: {}"
srv_wizard_input_prompt: "Nhập giá trị của bạn (Trống để chọn mặc định ["
srv_wizard_title: " FLAGS WIZARD (HỎI ĐÁP CỜ TÙY CHỌN DỊCH VỤ) "
srv_mode_terminal: "  1. Cấu hình cơ bản (Nhập văn bản - Simple Terminal)"
srv_mode_gui: "  2. Cấu hình cơ bản (Duyệt thư mục GUI - Simple GUI)"
srv_mode_advanced: "  3. Cấu hình nâng cao (Advanced Configuration)"
srv_select_mode_title: " THIẾT LẬP DỊCH VỤ {}: CHỌN CHẾ ĐỘ CẤU HÌNH "
srv_browser_info: " Remote: {} | Thư mục hiện tại: {}"
srv_browser_title: " BỘ DUYỆT THƯ MỤC {} (GUI SIMPLE) "
srv_browser_loading: "Đang tải danh sách thư mục từ Cloud..."
srv_browser_empty: "(Không có thư mục con nào - Bấm [Alt+N] để tạo mới thư mục)"
srv_browser_help: "[Up/Down]Di chuyển|[Enter]Vào thư mục|[Backspace]Lên một cấp|[Insert]Chọn thư mục này làm ĐÍCH|[Alt+N]Tạo thư mục con|[ESC]Thoát"
srv_browser_new_title: " TẠO THƯ MỤC MỚI TRÊN CLOUD "
srv_browser_new_prompt: "Nhập tên thư mục mới cần tạo:"
srv_browser_new_help: "[Enter] Xác nhận | [Esc] Hủy bỏ"
srv_action_level_user: "Cá nhân (User)"
srv_action_level_system: "Hệ thống (System)"
srv_action_title: " THAO TÁC DỊCH VỤ: {} ({}) "
srv_action_start: "1. Khởi động (Start)"
srv_action_stop: "2. Dừng (Stop)"
srv_action_restart: "3. Khởi động lại (Restart)"
srv_action_enable: "4. Bật tự khởi chạy cùng OS (Enable)"
srv_action_disable: "5. Tắt tự khởi chạy cùng OS (Disable)"
srv_action_edit: "6. Chỉnh sửa cấu hình (Edit)"
srv_systemd_edit_tab_basic: " [1] CƠ BẢN (SIMPLE) "
srv_systemd_edit_tab_adv: " [2] NÂNG CAO (ADVANCED) "
srv_systemd_edit_tab_help: "     (TAB/Trái/Phải để chuyển tab)"
srv_systemd_edit_save: "LƯU THAY ĐỔI / KHỞI TẠO"
srv_systemd_edit_cancel: "[ESC] HỦY BỎ"
srv_systemd_add_param_title: " THÊM THAM SỐ HỆ THỐNG MỚI "
srv_systemd_add_param_prompt: "Nhập Section và Tên khóa mới (ví dụ: [Service]LimitNOFILE):"
srv_systemd_add_param_help: "[Enter] Xác nhận | [Esc] Hủy bỏ"
srv_insert_gui_hint: " [Insert: Chọn GUI]"
srv_mount_fuse_missing_hint: " [Yêu cầu cài đặt FUSE]"

# Systemd Fields
sys_field__service_name_name: "Tên dịch vụ"
sys_field__service_name_desc: "Tên không kèm đuôi .service (ví dụ: rclone-torrent)"
sys_field__service_level_name: "Loại dịch vụ"
sys_field__service_level_desc: "User (Cá nhân) hoặc System (Hệ thống)"
sys_field__remote_name: "Cloud Remote (Nguồn)"
sys_field__remote_desc: "Nhập Remote (ví dụ: Main: hoặc Main:ThưMục)"
sys_field__mount_path_name: "Đường dẫn Mount cục bộ"
sys_field__mount_path_desc: "Đường dẫn thư mục trên máy tính của bạn"
sys_field__description_name: "Mô tả dịch vụ"
sys_field__description_desc: "Mô tả ngắn gọn về dịch vụ"
sys_field__user_name: "Tài khoản chạy"
sys_field__user_desc: "Tên tài khoản Linux chạy dịch vụ này"
sys_field_Unit_Description_name: "[Unit] Description"
sys_field_Unit_Description_desc: "Mô tả ngắn gọn về dịch vụ (Description)"
sys_field_Unit_After_name: "[Unit] After"
sys_field_Unit_After_desc: "Chạy sau khi các dịch vụ này khởi động (After)"
sys_field_Unit_Wants_name: "[Unit] Wants"
sys_field_Unit_Wants_desc: "Các dịch vụ muốn chạy cùng (Wants)"
sys_field_Unit_Requires_name: "[Unit] Requires"
sys_field_Unit_Requires_desc: "Yêu cầu các dịch vụ này chạy trước (Requires)"
sys_field_Unit_RequiresMountsFor_name: "[Unit] RequiresMountsFor"
sys_field_Unit_RequiresMountsFor_desc: "Chờ các thư mục này được mount trước (RequiresMountsFor)"
sys_field_Service_Type_name: "[Service] Type"
sys_field_Service_Type_desc: "Kiểu chạy dịch vụ (Type, vd: simple, fork, dbus)"
sys_field_Service_User_name: "[Service] User"
sys_field_Service_User_desc: "Tài khoản hệ thống chạy dịch vụ (User)"
sys_field_Service_Group_name: "[Service] Group"
sys_field_Service_Group_desc: "Nhóm tài khoản hệ thống chạy dịch vụ (Group)"
sys_field_Service_ExecStartPre_name: "[Service] ExecStartPre"
sys_field_Service_ExecStartPre_desc: "Lệnh chuẩn bị chạy trước ExecStart (ExecStartPre)"
sys_field_Service_ExecStart_name: "[Service] ExecStart"
sys_field_Service_ExecStart_desc: "Lệnh chính chạy dịch vụ rclone (ExecStart)"
sys_field_Service_ExecStop_name: "[Service] ExecStop"
sys_field_Service_ExecStop_desc: "Lệnh dọn dẹp khi dừng dịch vụ (ExecStop, vd: fusermount -u)"
sys_field_Service_Restart_name: "[Service] Restart"
sys_field_Service_Restart_desc: "Chế độ tự động khởi chạy lại khi gặp lỗi (Restart)"
sys_field_Service_RestartSec_name: "[Service] RestartSec"
sys_field_Service_RestartSec_desc: "Thời gian chờ trước khi restart (RestartSec)"
sys_field_Service_Environment_name: "[Service] Environment"
sys_field_Service_Environment_desc: "Khai báo biến môi trường (Environment)"
sys_field_Install_WantedBy_name: "[Install] WantedBy"
sys_field_Install_WantedBy_desc: "Khởi chạy dịch vụ ở runlevel nào (WantedBy)"

# Job Monitor
mon_speed_label: "Tốc độ hiện tại: "
mon_upload_speed_label: " | Tải lên: "
mon_download_speed_label: " | Tải xuống: "
mon_max_bandwidth_label: " | Băng thông tối đa: "
mon_transferred_label: " | Đã truyền tải: "
mon_total_pct_label: "Tiến trình tổng: "
mon_active_transfers_label: "Số luồng truyền tải (Transfers): "
mon_active_checkers_label: " | Số luồng kiểm tra (Checkers): "
mon_stats_title: " TỔNG QUAN TIẾN TRÌNH "
mon_active_title: " CÁC TÁC VỤ ĐANG TRUYỀN TẢI "
mon_history_title: " LỊCH SỬ HOẠT ĐỘNG (LẦN GẦN NHẤT) "
mon_action_checking: "Kiểm tra"
mon_help: "[Delete]Hủy bỏ (Stop) tác vụ đang chọn|[Mũi tên]Di chuyển|[ESC]Quay lại Menu chính"
lang_welcome: "Sử dụng phím Mũi tên Lên/Xuống để di chuyển, Enter để chọn/áp dụng, Esc để quay lại."
lang_active: "Đang dùng"
lang_title: " CÀI ĐẶT NGÔN NGỮ "
"#;

    let default_eng = r#"# Rclone-TUI English Translations
unikey_tip: "💡 Tip: If your IME interferes with typing, temporarily switch your OS layout to English mode."
remote: "Remote to encrypt/decrypt. Normally should contain a ':' and a path, e.g. \"myremote:path/to/dir\""
filename_encryption: "How to encrypt the filenames."
directory_name_encryption: "Option to either encrypt directory names or leave them intact."
password: "Password or pass phrase for encryption."
password2: "Password or pass phrase for salt. Optional but recommended. Should be different to the previous password."
client_id: "OAuth Client ID."
client_secret: "OAuth Client Secret."
description: "Description of the remote."
token: "OAuth access token."
filename_encoding: "How to encode the encrypted filename to text string."
no_data_encryption: "Option to either encrypt file data or leave it unencrypted."
pass_bad_blocks: "If set this will pass bad blocks through as all 0."
server_side_across_configs: "Allow server-side operations (e.g. copy) to work across different configs."
show_mapping: "For all files listed show how the names encrypt."
strict_names: "If set, this will raise an error when crypt comes across a filename that can't be decrypted."
suffix: "If this is set it will override the default suffix of \".bin\"."
upstreams: "List of space separated upstreams. Can be 'upstreama:test/dir upstreamb:', '\"upstreama:test/space:ro dir\" upstreamb:', etc."
action_policy: "Policy to choose upstream on ACTION category."
create_policy: "Policy to choose upstream on CREATE category."
search_policy: "Policy to choose upstream on SEARCH category."
cache_time: "Cache time of usage and free space (in seconds). This option is only useful when a path preserving policy is used."
min_free_space: "Minimum viable free space for lfs/eplfs policies. If a remote has less than this much free space then it won't be considered."
tip_select_remote: "💡 Use Up/Down or Left/Right arrows to select an existing remote"
tip_select_choice: "💡 Use Up/Down or Left/Right arrows to select a choice value"
help_editing: "[Enter] Finish editing | [ESC] Cancel editing"
help_navigation: "[Up/Down]Move|[Enter]Select/Edit|[Backspace]Delete parameter|[ESC]Cancel & Back"
help_general: "[Arrows/Tab]Move|[Enter]Select|[Space]Toggle checkbox|[ESC]Cancel & Back"
status_online: "🟢 Online"
status_online_loading: "🟢 Online (Calculating capacity...)"
status_online_unlimited: "🟢 Online (Unlimited)"
status_unchecked: "Unchecked"
confirm_delete_remote: "Are you sure you want to delete remote configuration '{}'?\n\n[Enter / Y] Confirm delete\n[Esc / N] Cancel"
confirm_delete_remote_title: " CONFIRM DELETE REMOTE "
confirm_delete_file: "Are you sure you want to delete '{}'?\nNote: The folder and all its contents will be permanently deleted!\n\n[Enter / Y] Confirm | [Esc / N] Cancel"
confirm_delete_file_title: " CONFIRM FILE DELETE "
confirm_delete_multiple: "Are you sure you want to delete {} selected items?\nNote: All selected folders and their contents will be permanently deleted!\n\n[Enter / Y] Confirm | [Esc / N] Cancel"
confirm_delete_multiple_title: " CONFIRM MULTIPLE FILES DELETE "
confirm_delete_service: "Are you sure you want to stop and remove background service:\n'{}'?\n\n[Enter / Y] Confirm | [Esc / N] Cancel"
confirm_delete_service_title: " CONFIRM BACKGROUND SERVICE KILL "
confirm_delete_systemd: "Are you sure you want to stop, disable, and PERMANENTLY DELETE systemd service:\n'{}'?\nThis will remove configuration from disk!\n\n[Enter / Y] Confirm | [Esc / N] Cancel"
confirm_delete_systemd_title: " CONFIRM SYSTEMD SERVICE DELETE "

conn_wizard_auth_headless: "3. Headless OAuth (Manual authorization for headless server/no browser)"
conn_wizard_headless_title: " HEADLESS OAUTH AUTHENTICATION ({}) "
conn_wizard_headless_prompt: "Run the following command on your local machine (with a web browser):\n\n  rclone authorize \"{}\"\n\nThen copy the resulting JSON token, paste it below and press Enter to confirm:"
exp_special_title: " SPECIAL ACTIONS MENU "
exp_special_link: "1. Get Public Link (operations/publiclink)"
exp_special_hash: "2. Calculate Checksum (operations/hashsumfile)"
exp_special_cleanup: "3. Clean up Cloud Trash (operations/cleanup)"
exp_special_rmdir: "4. Safe delete empty directory (rmdir)"
exp_special_rmdirs: "5. Safe delete recursive empty directories (rmdirs)"
exp_special_cryptdecode: "6. Decrypt Encrypted Filename (cryptdecode)"
exp_special_archive: "7. Extract Archive (archive)"
exp_special_dedupe: "8. Deduplicate files/folders (dedupe)"
exp_special_merge_similar: "9. Merge similar directories (Spaces/Case)"
exp_special_close: "[Esc] Close Menu"
exp_merge_similar_title: " MERGE SIMILAR DIRECTORIES "
exp_merge_similar_prompt: "Found {} groups of similar directories to merge:\n\n{}"
exp_merge_similar_help: "[Enter] Confirm merge | [Esc] Cancel"
exp_no_similar_dirs: "No similar directories found (differing by trailing spaces or case)!"
exp_dedupe_title: " DEDUPLICATE FILES / FOLDERS (DEDUPE) "
exp_dedupe_prompt: "Select how to resolve duplicate files:"
exp_dedupe_mode_rename: "1. Rename duplicates to keep all (Rename)"
exp_dedupe_mode_newest: "2. Keep newest, delete older (Keep newest)"
exp_dedupe_mode_oldest: "3. Keep oldest, delete newer (Keep oldest)"
exp_dedupe_mode_largest: "4. Keep largest, delete smaller (Keep largest)"
exp_dedupe_mode_smallest: "5. Keep smallest, delete larger (Keep smallest)"
exp_dedupe_mode_first: "6. Keep first found item (Keep first)"
exp_dedupe_mode_skip: "7. Skip all duplicate conflicts (Skip)"
exp_dedupe_by_hash_prompt: "Deduplication method:\n[Space] Toggle: Find by Hash instead of Name\nStatus: {}"
exp_dedupe_help: "[Up/Down] Select mode | [Space] Toggle Hash | [Enter] Start | [Esc] Cancel"
exp_copy_mode_title: " SELECT COPY/TRANSFER MODE "
exp_copy_mode_prompt: "Select how rclone compares source and destination files:"
exp_copy_mode_normal: "1. Normal Copy (Based on size and modification time)"
exp_copy_mode_checksum: "2. Checksum/Hash match (Recursive hash check, skip identical, overwrite different)"
exp_copy_mode_help: "[Up/Down] Select mode | [Enter] Confirm | [Esc] Cancel"
exp_hash_title: " SELECT CHECKSUM TYPE "
exp_hash_prompt: "Use Arrows and Enter to select checksum type:"
exp_cryptdecode_title: " DECRYPT FILENAME (CRYPTDECODE) "
exp_cryptdecode_prompt_remote: "1. Crypt Remote Name (e.g. mycrypt:):"
exp_cryptdecode_prompt_name: "2. Encrypted Name/Path to Decrypt:"
exp_cryptdecode_help: "[Tab] Switch fields | [Enter] Start decrypt | [Esc] Cancel"
exp_archive_title: " SELECT EXTRACTION MODE "
exp_archive_here: "1. Extract Here"
exp_archive_folder: "2. Extract to Folder"
exp_archive_path: "3. Extract to Path"
exp_archive_path_title: " SELECT DESTINATION METHOD "
exp_archive_path_manual: "1. Manual Path Input (Terminal-style)"
exp_archive_path_tui: "2. Browse Directories (TUI Explorer-style)"
exp_archive_manual_prompt: "Enter destination path for extraction:"
exp_archive_tui_prompt: "Browse remotes/folders and press [Insert] on target folder to extract into:"
exp_archive_tui_help: "[Up/Down]Move|[Enter]Enter dir|[Insert]Select|[Esc]Exit"
exp_paste_rename_title: " COPY AND RENAME (COPYTO) "
exp_paste_rename_prompt: "Enter new name for the item before pasting:"

# Main Menu
menu_1: "1. Connection Manager (Cloud Connections)"
menu_2: "2. File Explorer (Cloud Browser)"
menu_3: "3. Job Monitor (Sync Tasks)"
menu_4: "4. Config Profile Manager (Profiles)"
menu_5: "5. Services & Utilities (FUSE, Web GUI, Serve)"
menu_6: "6. Language Settings (Languages)"
menu_install_dep: "7. Install Dependencies (FUSE, Filen CLI)"
menu_7: "8. Exit"
menu_welcome: "Welcome to Rclone Clone TUI! Use Up/Down arrows to navigate and Enter to select."
menu_title: " MAIN MENU "
conn_insert_api_key_hint: " [Insert: Auto-fill API Key]"
conn_insert_api_key_missing_hint: " [Insert: Requires filen-cli]"

# Connection Manager
conn_help_navigation: "[Insert]Add remote|[Alt+E]Edit|[Delete]Delete remote|[?]Features|[Arrows]Move|[ESC]Main Menu"
conn_title: " CLOUD REMOTES "
conn_error_title: " ERROR "
conn_info_title: " INFO "
conn_wizard_provider_title: " STEP 1: SELECT CLOUD PROVIDERS (SPACE TO SELECT) "
conn_wizard_provider_configuring: "Configuring provider: "
conn_wizard_name_prompt: "Enter remote connection name:"
conn_wizard_name_title: " STEP 2: ENTER REMOTE NAME "
conn_wizard_auth_simple: "1. Simple OAuth (Auto open browser)"
conn_wizard_auth_advanced: "2. Advanced Configuration (Manual Client ID/Secret)"
conn_wizard_auth_mode_title: " STEP 3: SELECT AUTH MODE ({}) "
conn_wizard_oauth_title: " SIMPLE OAUTH AUTHENTICATION "
conn_wizard_oauth_started: "Simple OAuth auth for remote: "
conn_wizard_oauth_open_browser: "The application has attempted to open your web browser at the URL below."
conn_wizard_oauth_copy_url: "If browser does not open automatically, copy and paste this URL:"
conn_wizard_oauth_waiting: "WAITING FOR BROWSER AUTHENTICATION (Press ESC to cancel)..."
conn_wizard_edit_title: " ADVANCED CONFIG FOR REMOTE: {} ({}) "
conn_wizard_edit_title_simple: " EDIT REMOTE: {} ({}) "
conn_wizard_edit_tab_basic: " [1] BASIC "
conn_wizard_edit_tab_adv: " [2] ADVANCED "
conn_wizard_edit_tab_help: "     (Use TAB to switch tabs)"
conn_wizard_edit_save: "SAVE CHANGES"
conn_wizard_edit_cancel: "[ESC] BACK"
conn_remote_name_label: "Remote Name"

# File Explorer
exp_help: "[Tab]Pane|[Enter/BS]Enter/Back|[Alt+R]Remote|[Alt+Y]Rename|[Ctrl+C/V]Copy/Paste|[Ctrl+X]Move|[Delete]Delete|[Alt+N]New|[Alt+T]Sync|[Alt+V]Select|[Shift+V]Range|[Alt+O]Special|[ESC]Back"
exp_console_title: " STATUS / CONSOLE PANEL "
exp_console_empty: "No pending command (Press Ctrl+C to copy an item)"
exp_console_pending: "PENDING COPY from: {} (Press Ctrl+V to paste, Space to cancel)"
exp_copy_title: " COPY PROGRESS "
exp_copy_msg: "Copying file/folder...\nFrom: {}\nTo: {}\nProgress: {:.1}%\n[Press Ctrl+C to cancel]"
exp_move_title: " MOVE PROGRESS "
exp_move_msg: "Moving file/folder...\nFrom: {}\nTo: {}\nProgress: {:.1}%\n[Press Ctrl+C to cancel]"
exp_sync_title: " CONFIRM SYNC "
exp_sync_msg: "Are you sure you want to Sync one-way?\n\nSource (Active): {}{}\nTarget: {}{}\n\n[WARNING: This will delete files at TARGET if they don't exist at SOURCE!]\n\n[Enter / Y] Continue | [Esc / N] Cancel"
exp_loading: "  Loading data..."
exp_empty: "  (Empty folder)"
exp_creating_placeholder: "Creating..."
exp_new_folder_prompt: "Enter name of new folder:"
exp_new_folder_title: " CREATE NEW FOLDER "
exp_select_remote_title: " SELECT REMOTE STORAGE "
exp_add_shared_link_option: "[Add Shared Link (Google Drive)]"
exp_input_shared_link_prompt: "Enter Google Drive shared folder link:"
exp_select_base_remote_title: " SELECT YOUR BASE CREDENTIALS REMOTE "
exp_select_base_remote_prompt: "Select a Google Drive remote as credentials base:"
exp_permission_error_title: " ACCESS PERMISSION WARNING "
exp_permission_error_prompt: "Download permission restriction detected on this file!"
exp_permission_option_cancel: "Do not copy (Cancel)"
exp_permission_option_as_much: "Copy as much as possible (Skip failed files, keep directory structure)"
exp_permission_option_restricted: "Restricted copy (Only create folders containing successful files)"
exp_error_title: " EXPLORER ERROR "

# Config Profile Manager
prof_title: " CONFIG PROFILES MANAGER "
prof_help: "[Enter]Activate Profile|[Ctrl+X]Export Profile|[Insert]Import Profile|[ESC]Back"
prof_help_wizard: "[Arrows/Tab]Navigate|[Enter]Select|[ESC]Cancel"
prof_overwrite_import_msg: "Profile name '{}' already exists.\nDo you want to overwrite its config path?\n\n[Enter] Overwrite | [ESC] Cancel"
prof_overwrite_import_title: " CONFIRM IMPORT OVERWRITE "
prof_overwrite_export_msg: "Profile config '{}' already exists in Downloads/Saved Profile/.\nDo you want to overwrite it?\n\n[Enter] Overwrite | [ESC] Cancel"
prof_overwrite_export_title: " CONFIRM EXPORT OVERWRITE "
prof_export_success: "Export Profile successful!\nFile saved at:\n\n👉 {}\n\n[Press Enter or ESC to close]"
prof_success_title: " SUCCESS "
prof_error_title: " CONFIG ERROR "
prof_new_prompt: "Enter name of new Profile:"
prof_new_title: " STEP 1: PROFILE NAME "
prof_type_url: "1. Link Direct (Download from config URL)"
prof_type_local: "2. Copy & Pull (Copy from local config file)"
prof_type_title: " STEP 2: IMPORT METHOD ({}) "
prof_url_prompt: "Enter direct config URL:"
prof_file_prompt: "Enter local config file path (e.g. /home/user/rclone.conf):"
prof_importing: "Adding Profile: {}"
prof_source_title: " STEP 3: SOURCE CONFIG PATH "

# Services
srv_opt_mount: "1. Mount Virtual Drive via FUSE (FUSE virtual drive)"
srv_opt_nfsmount: "2. Mount Virtual Drive via NFS (NFS virtual drive)"
srv_opt_gui: "3. Open Web GUI (Rclone web interface)"
srv_opt_serve: "4. Quick File Sharing Server"
srv_title_config: " SERVICE CONFIGURATION "
srv_title_tui: " BACKGROUND SERVICES VIA TUI "
srv_tag_user: " [User]"
srv_tag_system: " [System]"
srv_title_systemd: " SYSTEMD SERVICES (RCLONE) "
srv_help_t0: "[Tab]Switch pane|[Enter]Select|[ESC]Back to Main Menu"
srv_help_t1: "[Tab]Switch pane|[Delete]Kill background TUI service|[ESC]Back to Main Menu"
srv_help_t2: "[Tab]Switch pane|[Enter]Edit config|[Space]Service action|[Insert]Add new|[Delete]Delete service|[ESC]Back"
srv_help_action: "[Arrows]Select action|[Enter]Execute|[ESC]Cancel"
srv_help_edit: "[Tab/Arrows]Switch Tab|[Up/Down]Move field|[Enter]Edit|[Delete]Delete key (Adv Tab)|[Insert]Add key (Adv Tab)|[ESC]Cancel"
srv_help_general: "[Tab/Arrows]Navigate|[Enter]Select|[ESC]CANCEL service setup"
srv_local_system: "[Local System]"
srv_edit_systemd_title: "EDIT SYSTEMD SERVICE: {}"
srv_new_systemd_title: "CREATE NEW SYSTEMD SERVICE (RCLONE MOUNT)"
srv_error_title: " SERVICE ERROR "
srv_info_title: " INFO "
srv_local_desc: "  [Local] -> Use local system memory/disk"
srv_cloud_desc: "  [Cloud] -> {}"
srv_select_source_title: " SERVICE SETUP {}: SELECT DATA SOURCE "
srv_mount_point_prompt: "Enter local MOUNT POINT path (e.g. /home/user/mnt):"
srv_share_path_prompt: "Enter subdirectory path inside remote to share (Blank to share all):"
srv_selected_source: "Selected data source: "
srv_mount_point_title: " DIRECTORY / MOUNT POINT SETUP "
srv_proto_http: "1. http (Easy, view via browser)"
srv_proto_ftp: "2. ftp (FTP clients compatible)"
srv_proto_webdav: "3. webdav (WebDAV network drive compatible)"
srv_proto_sftp: "4. sftp (Secure, encrypted traffic)"
srv_select_proto_title: " SELECT SHARE PROTOCOL ({}{}) "
srv_config_for: "Configure service for: "
srv_wizard_progress: "Flag configuration progress: {} / {}"
srv_wizard_flag: "Flag: {}"
srv_wizard_input_prompt: "Enter your value (Blank to select default ["
srv_wizard_title: " FLAGS WIZARD (SERVICE FLAGS SETUP) "
srv_mode_terminal: "  1. Basic config (Simple Terminal)"
srv_mode_gui: "  2. Basic config (Simple GUI)"
srv_mode_advanced: "  3. Advanced Configuration"
srv_select_mode_title: " SERVICE SETUP {}: SELECT CONFIG MODE "
srv_browser_info: " Remote: {} | Current path: {}"
srv_browser_title: " DIRECTORY BROWSER {} (SIMPLE GUI) "
srv_browser_loading: "Loading directory list from Cloud..."
srv_browser_empty: "(No subdirectories - Press [Alt+N] to create new folder)"
srv_browser_help: "[Up/Down]Move|[Enter]Enter dir|[Backspace]Go Up|[Insert]Select this dir as TARGET|[Alt+N]Create folder|[ESC]Exit"
srv_browser_new_title: " CREATE NEW FOLDER ON CLOUD "
srv_browser_new_prompt: "Enter name of new folder:"
srv_browser_new_help: "[Enter] Confirm | [Esc] Cancel"
srv_action_level_user: "User"
srv_action_level_system: "System"
srv_action_title: " SERVICE ACTION: {} ({}) "
srv_action_start: "1. Start"
srv_action_stop: "2. Stop"
srv_action_restart: "3. Restart"
srv_action_enable: "4. Enable auto-start with OS"
srv_action_disable: "5. Disable auto-start with OS"
srv_action_edit: "6. Edit Configuration"
srv_systemd_edit_tab_basic: " [1] SIMPLE "
srv_systemd_edit_tab_adv: " [2] ADVANCED "
srv_systemd_edit_tab_help: "     (TAB/Left/Right to switch tab)"
srv_systemd_edit_save: "SAVE CHANGES / INITIALIZE"
srv_systemd_edit_cancel: "[ESC] CANCEL"
srv_systemd_add_param_title: " ADD NEW SYSTEM PARAMETER "
srv_systemd_add_param_prompt: "Enter Section and new key name (e.g. [Service]LimitNOFILE):"
srv_systemd_add_param_help: "[Enter] Confirm | [Esc] Cancel"
srv_insert_gui_hint: " [Insert: Select GUI]"
srv_mount_fuse_missing_hint: " [FUSE is required]"


# Systemd Fields
sys_field__service_name_name: "Service Name"
sys_field__service_name_desc: "Name without .service suffix (e.g. rclone-torrent)"
sys_field__service_level_name: "Service Level"
sys_field__service_level_desc: "User (Personal) or System"
sys_field__remote_name: "Cloud Remote (Source)"
sys_field__remote_desc: "Enter remote (e.g. Main: or Main:Folder)"
sys_field__mount_path_name: "Local Mount Path"
sys_field__mount_path_desc: "Directory path on your computer"
sys_field__description_name: "Service Description"
sys_field__description_desc: "Short description of the service"
sys_field__user_name: "Run User"
sys_field__user_desc: "Linux user running this service"
sys_field_Unit_Description_name: "[Unit] Description"
sys_field_Unit_Description_desc: "Short description of the service (Description)"
sys_field_Unit_After_name: "[Unit] After"
sys_field_Unit_After_desc: "Run after these services start (After)"
sys_field_Unit_Wants_name: "[Unit] Wants"
sys_field_Unit_Wants_desc: "Services wanted to run together (Wants)"
sys_field_Unit_Requires_name: "[Unit] Requires"
sys_field_Unit_Requires_desc: "Require these services to run first (Requires)"
sys_field_Unit_RequiresMountsFor_name: "[Unit] RequiresMountsFor"
sys_field_Unit_RequiresMountsFor_desc: "Wait for these directories to mount (RequiresMountsFor)"
sys_field_Service_Type_name: "[Service] Type"
sys_field_Service_Type_desc: "Service run type (Type, e.g. simple, fork, dbus)"
sys_field_Service_User_name: "[Service] User"
sys_field_Service_User_desc: "System account running the service (User)"
sys_field_Service_Group_name: "[Service] Group"
sys_field_Service_Group_desc: "System group running the service (Group)"
sys_field_Service_ExecStartPre_name: "[Service] ExecStartPre"
sys_field_Service_ExecStartPre_desc: "Command running before ExecStart (ExecStartPre)"
sys_field_Service_ExecStart_name: "[Service] ExecStart"
sys_field_Service_ExecStart_desc: "Main command running rclone service (ExecStart)"
sys_field_Service_ExecStop_name: "[Service] ExecStop"
sys_field_Service_ExecStop_desc: "Clean up command when stopping (ExecStop, e.g. fusermount -u)"
sys_field_Service_Restart_name: "[Service] Restart"
sys_field_Service_Restart_desc: "Auto-restart policy on failure (Restart)"
sys_field_Service_RestartSec_name: "[Service] RestartSec"
sys_field_Service_RestartSec_desc: "Delay before restart (RestartSec)"
sys_field_Service_Environment_name: "[Service] Environment"
sys_field_Service_Environment_desc: "Environment variables declaration (Environment)"
sys_field_Install_WantedBy_name: "[Install] WantedBy"
sys_field_Install_WantedBy_desc: "Which runlevel to target (WantedBy)"

# Job Monitor
mon_speed_label: "Current Speed: "
mon_upload_speed_label: " | Upload: "
mon_download_speed_label: " | Download: "
mon_max_bandwidth_label: " | Max Bandwidth: "
mon_transferred_label: " | Transferred: "
mon_total_pct_label: "Total Progress: "
mon_active_transfers_label: "Transfers: "
mon_active_checkers_label: " | Checkers: "
mon_stats_title: " GLOBAL STATS "
mon_active_title: " ACTIVE JOBS "
mon_history_title: " ACTIVITY HISTORY (RECENT) "
mon_action_checking: "Checking"
mon_help: "[Delete]Stop selected job|[Arrows]Move|[ESC]Back to Main Menu"
lang_welcome: "Use Arrow keys Up/Down to navigate, Enter to select/apply, Esc to return."
lang_active: "Active"
lang_title: " LANGUAGE SETTINGS "
"#;

    let vn_path = lang_dir.join("vn.yaml");
    merge_missing_keys(&vn_path, default_vn);

    let eng_path = lang_dir.join("eng.yaml");
    merge_missing_keys(&eng_path, default_eng);
}

fn merge_missing_keys(path: &std::path::Path, defaults: &str) {
    let mut current_map: HashMap<String, String> = if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            serde_yaml::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    if let Ok(default_map) = serde_yaml::from_str::<HashMap<String, String>>(defaults) {
        let mut modified = false;
        for (k, v) in default_map {
            let is_help_key = k.ends_with("_help") || k.ends_with("_save") || k.ends_with("_cancel") || k.ends_with("_friendly") || k.starts_with("menu_") || k == "conn_help_navigation" || k == "mon_help" || k == "prof_help" || k == "prof_help_wizard" || k == "srv_help_edit" || k == "srv_insert_gui_hint" || k == "conn_insert_api_key_hint" || k == "conn_insert_api_key_missing_hint" || k == "srv_mount_fuse_missing_hint";
            if !current_map.contains_key(&k) || is_help_key {
                if current_map.get(&k) != Some(&v) {
                    current_map.insert(k, v);
                    modified = true;
                }
            }
        }
        if modified || !path.exists() {
            if let Ok(new_content) = serde_yaml::to_string(&current_map) {
                let _ = fs::write(path, new_content);
            }
        }
    }
}

/// Nạp ngôn ngữ từ file YAML vào bộ nhớ
pub fn load_translation(lang_name: &str) {
    let lang_dir = crate::app_config::AppConfig::config_dir().join("lang");
    let file_path = lang_dir.join(format!("{}.yaml", lang_name));

    if let Ok(content) = fs::read_to_string(&file_path) {
        if let Ok(map) = serde_yaml::from_str::<HashMap<String, String>>(&content) {
            let mut trans = TRANSLATIONS.write().unwrap();
            *trans = map;
            return;
        }
    }

    // Nếu không nạp được file cụ thể, tự tạo dự phòng mặc định tùy theo tên ngôn ngữ
    let mut fallback = HashMap::new();
    if lang_name == "vn" {
        fallback.insert("unikey_tip".to_string(), "💡 Mẹo: Nếu Unikey tự chuyển dấu sai (ví dụ Telex), hãy tắt bộ gõ tiếng Việt (chuyển sang chữ E) trong hệ điều hành.".to_string());
        fallback.insert(
            "remote".to_string(),
            "Remote nguồn cần mã hóa (ví dụ: Telebox:ThuMucGoc)".to_string(),
        );
    } else {
        fallback.insert("unikey_tip".to_string(), "💡 Tip: If your IME interferes with typing, temporarily switch your OS layout to English mode.".to_string());
        fallback.insert(
            "remote".to_string(),
            "Remote to encrypt/decrypt. Normally should contain a ':' and a path".to_string(),
        );
    }
    let mut trans = TRANSLATIONS.write().unwrap();
    *trans = fallback;
}

/// Quét thư mục lang để lấy danh sách các ngôn ngữ khả dụng (.yaml)
pub fn get_available_languages() -> Vec<String> {
    let lang_dir = crate::app_config::AppConfig::config_dir().join("lang");
    let mut langs = Vec::new();

    if let Ok(entries) = fs::read_dir(lang_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    langs.push(stem.to_string());
                }
            }
        }
    }

    // Nếu rỗng, thêm mặc định
    if langs.is_empty() {
        langs.push("vn".to_string());
        langs.push("eng".to_string());
    }

    langs.sort();
    langs
}

/// Hàm dịch tổng quát theo key từ tệp cấu hình ngôn ngữ
pub fn translate(key: &str) -> String {
    let trans = TRANSLATIONS.read().unwrap();
    if let Some(val) = trans.get(key) {
        let mut val_str = val.clone();
        if cfg!(target_os = "macos") {
            val_str = val_str.replace("Alt+", "Ctrl+");
        }
        return val_str;
    }
    key.to_string()
}

/// Dịch mô tả tham số của rclone
pub fn translate_desc(field_name: &str, english_desc: &str) -> String {
    let trans = TRANSLATIONS.read().unwrap();
    if let Some(val) = trans.get(field_name) {
        return val.clone();
    }

    // Thuật toán dịch một số cụm từ cơ bản nếu không tìm thấy key chính xác
    let mut translated = english_desc.to_string();
    if translated.contains("OAuth Client ID") {
        translated = translated.replace("OAuth Client ID", "OAuth Client ID của ứng dụng");
    }
    if translated.contains("OAuth Client Secret") {
        translated = translated.replace("OAuth Client Secret", "OAuth Client Secret của ứng dụng");
    }
    if translated.contains("Password") {
        translated = translated.replace("Password", "Mật khẩu");
    }
    translated
}

/// Dịch các mẹo/lưu ý trên giao diện
pub fn translate_tip(tip_key: &str) -> String {
    translate(tip_key)
}

/// Dịch tên thân thiện của các tùy chọn cấu hình
pub fn translate_friendly(field_name: &str) -> Option<String> {
    let trans = TRANSLATIONS.read().unwrap();
    let key = format!("{}_friendly", field_name);
    trans.get(&key).cloned()
}
