---
name: github-free-tier-solve
description: Sử dụng khi cấu hình, gỡ lỗi và tối ưu hóa quy trình CI/CD (GitHub Actions) trên môi trường runner miễn phí (Free Tier). Chứa các quy chuẩn xử lý biên dịch chéo CGO, đồng bộ hóa toolchain trên Windows (MSVC vs GNU), sửa lỗi đóng gói Linux (AppImage, deb, rpm), lỗi thụt lề heredoc, và quản lý các giới hạn tài nguyên máy ảo.
user-invocable: false
---

# Quy Chuẩn Giải Quyết Lỗi CI/CD Trên GitHub Actions Free Tier (`github-free-tier-solve`)

Tài liệu này tổng hợp các kinh nghiệm thực tế và giải pháp kỹ thuật để xử lý các lỗi tương thích hệ điều hành, kiến trúc CPU và trình liên kết (linker) khi build các dự án đa ngôn ngữ (Rust + Go CGO) trên môi trường ảo hóa miễn phí của GitHub Actions.

---

## 1. Giới Hạn Phần Cứng & Rào Cản Biên Dịch Chéo (Cross-Compilation)

### 1.1. Bản chất runner của GitHub Actions Free Tier
* Các máy ảo chạy hệ điều hành `ubuntu-latest` và `windows-latest` trong gói miễn phí hoàn toàn chạy trên **vi xử lý kiến trúc x86_64 (Intel/AMD)**.
* **Hệ quả**: Để tạo ra các bản phân phối chạy trên kiến trúc ARM64 (Linux ARM64, Windows ARM64), chúng ta **buộc phải biên dịch chéo (cross-compile)** thay vì biên dịch native.

### 1.2. Rào cản CGO khi biên dịch chéo
* Khi dự án sử dụng các thư viện Go có kích hoạt CGO (`CGO_ENABLED=1`) để tạo static library (`-buildmode=c-archive`), Go yêu cầu một trình biên dịch C (GCC/Clang) đích tương thích với kiến trúc CPU đích.
* **Linux ARM64**: Cross-compile rất dễ dàng bằng cách cài đặt gói `gcc-aarch64-linux-gnu` trên Ubuntu runner và thiết lập `CC=aarch64-linux-gnu-gcc`.
  * **Windows ARM64**: Biên dịch chéo sang target `aarch64-pc-windows-gnullvm` (MinGW + LLVM) thay vì `msvc` để đồng nhất với Go CGO build. Việc cài đặt và cấu hình toolchain rất dễ gặp lỗi do đường dẫn cài đặt ảo hóa và xung đột chế độ Clang driver (MSVC mode vs MinGW mode).

---

## 2. Đồng Bộ Hóa Toolchain Windows: Tránh Trộn Lẫn MSVC và GNU/GNULLVM

Khi liên kết tĩnh thư viện viết bằng Go CGO vào dự án Rust trên Windows, việc trộn lẫn trình biên dịch sẽ gây ra các lỗi liên kết nghiêm trọng.

### 2.1. Các lỗi thường gặp khi trộn lẫn toolchain (MinGW Go + MSVC Rust)
1. **Lỗi `LNK1223: invalid or corrupt file: file contains invalid .pdata contributions`**:
   * *Nguyên nhân*: Go CGO trên Windows bắt buộc phải dùng GCC (MinGW-w64) để build. Trình liên kết MSVC (`link.exe`) của Rust target `x86_64-pc-windows-msvc` không thể đọc đúng cấu trúc exception handling / unwind table (`.pdata`) do GCC tạo ra cho kiến trúc 64-bit.
2. **Lỗi `LNK2019: unresolved external symbol fprintf referenced in function _cgo_beginthread`**:
   * *Nguyên nhân*: Hàm `fprintf` được inline hoàn toàn vào header trong modern MSVC CRT (UCRT), nhưng file object sinh ra bởi GCC vẫn coi nó là một biểu tượng (symbol) liên kết ngoài.

### 2.2. Giải pháp cho Windows x64: Sử dụng Target Windows GNU (`x86_64-pc-windows-gnu`)
Để giải quyết triệt để tất cả các xung đột ABI trên, ta chuyển toàn bộ quy trình biên dịch trên Windows sang hệ sinh thái **GNU (MinGW)**:

* **Thay đổi Target của Rust**: Thay thế `x86_64-pc-windows-msvc` bằng **`x86_64-pc-windows-gnu`**.
* **Đồng nhất Trình biên dịch**: Cả Go CGO và Rust đều sử dụng GCC được cung cấp bởi bước cấu hình MSYS2 MinGW:
  ```yaml
  - name: Set up MinGW (Windows)
    if: matrix.target == 'x86_64-pc-windows-gnu'
    uses: msys2/setup-msys2@v2
    with:
      update: true
      install: mingw-w64-x86_64-toolchain

  - name: Add GCC to PATH (Windows)
    if: matrix.target == 'x86_64-pc-windows-gnu'
    run: |
      echo "C:\msys64\mingw64\bin" >> $env:GITHUB_PATH
  ```
* **Định dạng file thư viện đầu ra của Go**: Đổi tên file static library từ `rclone.lib` thành **`librclone.a`** để trình liên kết GCC tự động nhận diện chuẩn xác.

### 2.3. Hướng dẫn cấu hình Windows ARM64 (`aarch64-pc-windows-gnullvm`)

Để build Windows ARM64 thành công trên runner x86_64 mà không dùng MSVC, ta sử dụng target `gnullvm` cùng bộ Clang/LLVM từ MSYS2. Dưới đây là các lưu ý quan trọng để tránh lỗi:

#### 1. Dùng đường dẫn động của MSYS2 thay vì ổ cứng (`C:\msys64`)
* **Lỗi**: `cgo: C compiler "C:/msys64/clangarm64/bin/clang" not found`.
* **Nguyên nhân**: Trên Windows runner của GitHub Actions, MSYS2 có thể được cài tại `D:\a\msys64` (hoặc thư mục temp chạy) thay vì `C:\msys64`.
* **Giải pháp**: Gán `id` cho step setup MSYS2 và sử dụng output `${{ steps.msys2.outputs.msys2-location }}`:
  ```yaml
  - name: Set up MinGW (Windows ARM64)
    id: msys2
    uses: msys2/setup-msys2@v2
    with:
      msystem: CLANGARM64
      update: true
      install: mingw-w64-clang-aarch64-toolchain

  - name: Add GCC/Clang to PATH (Windows ARM64)
    shell: pwsh
    run: |
      "$("${{ steps.msys2.outputs.msys2-location }}")\clangarm64\bin" | Out-File -FilePath $env:GITHUB_PATH -Append
  ```

#### 2. Chỉ định tuyệt đối đường dẫn Clang compiler của MSYS2
* **Lỗi**: `lld-link: error: could not open 'unwind.lib'` (hoặc `mingw32.lib`, `mingwex.lib`).
* **Nguyên nhân**: Lệnh `clang` mặc định trên runner trỏ về phiên bản MSVC-mode của Visual Studio (dẫn tới việc dùng `lld-link` tìm các file `.lib` giả định).
* **Giải pháp**: Sử dụng shell `bash` để tránh lỗi cú pháp và trỏ trực tiếp biến `CC` cùng linker của Rust về Clang của MSYS2:
  ```yaml
  # Cấu hình cho Go CGO
  env:
    CC: ${{ steps.msys2.outputs.msys2-location }}/clangarm64/bin/clang
  
  # Cấu hình trình liên kết (linker) cho Cargo
  env:
    CARGO_TARGET_AARCH64_PC_WINDOWS_GNULLVM_LINKER: ${{ steps.msys2.outputs.msys2-location }}/clangarm64/bin/clang
  ```

---

## 3. Sửa Lỗi Đóng Gói Ứng Dụng Trên Linux (deb, rpm, AppImage)

### 3.1. Lỗi khoảng trắng thừa trong Heredoc (`cat <<EOF`)
* Khi viết script ghi đè file cấu hình hoặc file `.desktop` trực tiếp trong YAML workflow thông qua cú pháp heredoc, việc thụt lề code (indentation) để định dạng YAML đẹp mắt sẽ vô tình chèn khoảng trắng thừa vào đầu mỗi dòng của file đích.
* **Hậu quả**: Trình đóng gói `appimagetool` sẽ báo lỗi và dừng build ngay lập tức nếu file `.desktop` chứa khoảng trắng ở đầu dòng:
  ```text
  line "  [Desktop Entry]" starts with a space ...
  ERROR: Desktop file contains errors. Please fix them.
  ```
* **Quy tắc khắc phục**: Tất cả nội dung nằm trong khối heredoc và thẻ đóng `EOF` phải được viết sát lề trái, căn chỉnh chính xác theo mức thụt lề cơ bản của block `run: |` (thường là 10 khoảng trắng):
  ```yaml
        run: |
          cat > AppDir/rclone-tui.desktop <<EOF
          [Desktop Entry]
          Type=Application
          Name=Rclone TUI
          Exec=rclone-tui
          Icon=rclone-tui
          Categories=Utility;
          EOF
  ```

### 3.2. Cài đặt Rust Toolchain an toàn
* Tránh sử dụng action cài đặt Rust toolchain mà không chỉ định thuộc tính `toolchain` rõ ràng vì một số phiên bản action yêu cầu bắt buộc tham số đầu vào này.
* **Cấu hình chuẩn**:
  ```yaml
  - name: Set up Rust
    uses: dtolnay/rust-toolchain@stable
    with:
      toolchain: stable
      targets: ${{ matrix.target }}
  ```

### 3.3. Tận dụng gói `alien` để đóng gói RPM nhanh chóng
* Thay vì viết riêng các script cấu hình `.spec` phức tạp để build gói `.rpm`, ta có thể build ra gói cài đặt `.deb` bằng lệnh `dpkg-deb --build` chuẩn, sau đó sử dụng công cụ `alien` trên Ubuntu để chuyển đổi tự động:
  ```bash
  sudo apt-get install -y alien rpm
  # Chuyển đổi gói deb sang rpm
  sudo alien --to-rpm rclone-tui-linux-amd64.deb
  ```
