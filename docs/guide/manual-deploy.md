# 手动部署

## 编译二进制

### Linux/macOS

```bash
# 克隆项目
git clone https://github.com/Maple0517/reader-next.git
cd reader-next

# 编译发布版本
cargo build --release

# 二进制在 target/release/reader-rust
```

### 静态链接

如需静态链接（便于分发）：

```bash
# Linux x86
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

```bash
# linux aarch64
rustup target add aarch64-unknown-linux-musl
cargo build --release --target aarch64-unknown-linux-musl
```
