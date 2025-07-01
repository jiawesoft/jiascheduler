# 构建阶段：构建后端
FROM rust:latest AS backend-builder
WORKDIR /app

# 复制 Rust 依赖文件，以便利用缓存
# COPY Cargo.toml Cargo.lock ./

# 创建 src 目录，防止 cargo build 失败
# RUN mkdir src && echo "fn main() {}" > src/main.rs

# 预先构建依赖，缓存编译结果
# RUN cargo build --release --verbose || true

# 复制前端编译产物到后端的 dist 目录
COPY dist /app/dist

# 复制后端代码并编译
COPY ./ ./
RUN cargo build --release

# 第二阶段：构建最终运行环境
FROM ubuntu:latest
WORKDIR /app

# 安装必要依赖
RUN apt update && apt install -y ca-certificates

# 设置时区环境变量
ENV TZ=Asia/Shanghai

# 安装 tzdata 包并配置时区（非交互模式）
RUN DEBIAN_FRONTEND=noninteractive apt-get install -y tzdata && \
    ln -fs /usr/share/zoneinfo/$TZ /etc/localtime && \
    echo $TZ > /etc/timezone && \
    apt-get clean && \
    dpkg-reconfigure --frontend noninteractive tzdata && \
    rm -rf /var/lib/apt/lists/*


# 复制后端可执行文件
COPY --from=backend-builder /app/target/release/jiascheduler /app/
COPY --from=backend-builder /app/target/release/jiascheduler-console /app/
COPY --from=backend-builder /app/target/release/jiascheduler-comet /app/
COPY --from=backend-builder /app/target/release/jiascheduler-agent /app/

# 设置运行时环境变量（如有需要）
ENV RUST_LOG=info

# 暴露必要端口
EXPOSE 9090 3000

# 启动命令（默认启动 jiascheduler-console）
CMD ["./jiascheduler-console", "--bind-addr", "0.0.0.0:9090"]
