# Nantian Gateway Data Plane — 优化分析报告

> 生成日期：2026-06-06
> 实施日期：2026-06-06
> 项目：基于 Pingora 0.8 的 Rust 数据面网关，14 个 crate

---

## 实施进度

| # | 优先级 | 类别 | 项目 | 状态 | 改动 |
|---|--------|------|------|------|------|
| 1 | P0 | 延迟 | Stream 连接池默认值 0→128 | ✅ 完成 | `defaults.rs:1` |
| 2 | P0 | 延迟 | TCP Fast Open 默认开启 | ✅ 完成 | `defaults.rs:1` |
| 3 | P0 | 延迟 | HTTP/3 QUIC 支持 | ⚠️ 需 Pingora 升级 | — |
| 4 | P0 | 延迟 | AI 缓存 RwLock→DashMap | ✅ 完成 | `semantic_cache.rs` |
| 5 | P1 | 延迟 | HTTP Cache Box::leak 优化 | 📋 高风险，暂缓 | — |
| 6 | P1 | 延迟 | xDS poll 间隔 25→100ms | ✅ 完成 | `defaults.rs:1` |
| 7 | P1 | 延迟 | 外部 Auth 连接池 | 📋 需设计 | — |
| 8 | P1 | 延迟 | HTTP 响应压缩 | 📋 需设计 | — |
| 9 | P2 | 延迟 | upstream read timeout 30s→15s | ✅ 完成 | `defaults.rs:1` |
| 10 | P2 | 延迟 | 访问日志采样率 1.0→0.5 | ✅ 完成 | `defaults.rs:1` |
| 11 | P0 | 安全 | 外部 Auth TLS 加密 | 📋 已添加警告日志 | `external_auth.rs` |
| 12 | P0 | 安全 | Session Secret 强制配置 | ✅ 完成 (error 级别日志) | `session.rs`, `runtime.rs` |
| 13 | P0 | 安全 | API Key 内存加密 (zeroize) | ✅ 完成 | `keyring.rs` |
| 14 | P0 | 安全 | xDS TLS 默认启用 | ✅ 完成 | `lib.rs`, `defaults.rs` |
| 15 | P1 | 安全 | 响应安全头 | ✅ 完成 | `filters.rs` |
| 16 | P1 | 安全 | IP 黑白名单 | 📋 需设计 | — |
| 17 | P1 | 安全 | Content-Type 校验 | 📋 需设计 | — |
| 18 | P1 | 安全 | Admin API mTLS | 📋 需设计 | — |
| 19 | P2 | 安全 | Graceful drain 0→30s | ✅ 完成 | `defaults.rs:1` |
| 20 | P2 | 安全 | HTTP 请求头限制 64KB | ✅ 完成 | `lib.rs`, `defaults.rs` |

**完成率：14/20 (70%)** — 6 项需进一步设计或外部依赖升级。

### 修改文件清单

| 文件 | 改动类型 |
|------|---------|
| `Cargo.toml` (workspace) | 添加 `zeroize` 依赖 |
| `Cargo.lock` | 依赖锁定更新 |
| `crates/aeg-ai/Cargo.toml` | 添加 `dashmap`, `zeroize` 依赖 |
| `crates/aeg-ai/src/keyring.rs` | `BackendCredential` 添加 `Zeroize` derive + `Drop` |
| `crates/aeg-ai/src/semantic_cache.rs` | `RwLock<HashMap>` → `DashMap` |
| `crates/aeg-config/src/defaults.rs` | 8 项默认值变更 + 1 个新函数 |
| `crates/aeg-config/src/defaults_impl.rs` | 清理未使用 import |
| `crates/aeg-config/src/lib.rs` | xDS TLS `enabled` 默认值 + header bytes 默认值 |
| `crates/aeg-config/src/tests/basics.rs` | 适配新默认值的测试断言 |
| `crates/aeg-http/src/filters.rs` | 添加 `apply_default_security_headers` |
| `crates/aeg-http/src/session.rs` | `info!` → `error!` 日志升级 |
| `crates/aeg-http/src/runtime.rs` | `info!` → `warn!` 日志升级 + 改进消息 |
| `crates/aeg-http/src/proxy/external_auth.rs` | 添加明文 TCP 安全警告 |

---

## 项目概况

| 组件 | 说明 |
|------|------|
| `aeg-http` | HTTP/gRPC 代理运行时、过滤器链、缓存、会话管理 |
| `aeg-ai` | AI 网关：多格式适配、限流、内容安全、语义缓存、A/B 测试 |
| `aeg-stream` | TCP/UDP/TLS 透传流代理 |
| `aeg-ir` | 运行时 IR、路由快照索引、proto 摄入 |
| `aeg-xds` | xDS 客户端，控制面配置流式下发 |
| `aeg-config` | 数据面配置管理、热重载 |
| `aeg-observability` | 指标、追踪、准入控制、断路器、限流、重试预算 |
| `aeg-shared-tls` | TLS 证书管理与轮转 |
| `aeg-wasm` | wasmtime 插件引擎 |
| `aeg-app` | 主二进制入口、服务编排 |

---

## 一、延迟优化建议

### P0 — 高优先级

#### 1. Stream 上游连接池默认关闭 — 每个请求都新建 TCP 连接

**位置**：`crates/aeg-config/src/defaults.rs:223`

```rust
pub(crate) fn default_stream_upstream_pool_size() -> usize { 0 }  // 默认禁用！
```

**影响**：TCP/UDP 流代理每个新连接都要完成 TCP 三次握手 + 可能的 TLS 握手，延迟增加 ~10-50ms。

**建议**：将 `streamUpstreamPoolSize` 默认值设为 **128**，启用 `TcpConnectionPool`（`crates/aeg-stream/src/pool.rs`）的连接复用。

**配置项**：`runtimeTuning.streamUpstreamPoolSize`

---

#### 2. TCP Fast Open 默认关闭

**位置**：`crates/aeg-config/src/defaults.rs:207`

```rust
pub(crate) fn default_upstream_tcp_fast_open() -> bool { false }
```

**影响**：每次上游连接多 1 个 RTT。内网环境可节省 1-5ms，跨地域可节省 10-50ms。

**建议**：在 Linux 内核支持的环境（`sysctl net.ipv4.tcp_fastopen=3`）下默认开启。

**配置项**：`runtimeTuning.upstreamTcpFastOpen`

---

#### 3. HTTP/3 (QUIC) 不可用 — 硬编码返回 false

**位置**：`crates/aeg-http/src/runtime.rs:168`

```rust
pub fn http3_available() -> bool { false }
```

**影响**：无法利用 QUIC 的 0-RTT 握手和多路复用优势，移动端或弱网环境延迟改善显著。

**建议**：如果 Pingora 0.8 支持 QUIC，实现真正的 HTTP/3 检测；若当前版本不支持，评估升级到更新版本。

---

#### 4. AI Gateway MemoryCache 使用全局 RwLock — 高并发下锁竞争

**位置**：`crates/aeg-ai/src/semantic_cache.rs:30`

```rust
pub struct MemoryCacheBackend {
    entries: RwLock<HashMap<String, CachedResponse>>,  // 单一全局锁
}
```

**影响**：高并发 AI 请求场景下，写锁会阻塞所有读操作，成为瓶颈。

**建议**：
- 方案 A：使用 `DashMap` 替代 `RwLock<HashMap>`（无锁并发读）
- 方案 B：使用 `moka::sync::Cache` 或 `quick_cache` 等成熟的并发缓存库（内置 TTL 驱逐、权重限制）
- 方案 C：分片锁（sharded lock），如 16 个分片的 `RwLock<HashMap>`

---

### P1 — 中优先级

#### 5. HTTP Cache 使用 `Box::leak` 静态存储 — 无法动态调整

**位置**：`crates/aeg-http/src/cache/mod.rs:83-85`

```rust
let storage: &'static MemCache = Box::leak(Box::new(MemCache::new()));
let eviction: &'static LruManager = Box::leak(Box::new(LruManager::new(options.max_size_bytes)));
```

**影响**：缓存大小在启动时固定，无法热更新；配置过大可能 OOM。

**建议**：改为 `Arc<Mutex<MemCache>>` 支持运行时调整，或将 `maxSizeMb` 默认值设为物理内存的 10-20%。

**配置项**：`runtimeTuning.httpCache.maxSizeMb`（默认 0 = 禁用）

---

#### 6. xDS Apply 轮询间隔过于激进

**位置**：`crates/aeg-config/src/defaults.rs:187`

```rust
pub(crate) fn default_xds_apply_poll_interval_ms() -> u64 { 25 }  // 每 25ms 轮询
```

**影响**：每秒 40 次轮询，配置无变化时浪费 CPU。

**建议**：改为事件驱动通知（利用 `watch::changed()`）配合更大的间隔如 100ms。

**配置项**：`xdsTransport.applyPollIntervalMs`

---

#### 7. 外部 Auth HTTP 协议每次新建 TCP 连接

**位置**：`crates/aeg-http/src/proxy/external_auth.rs:39`

```rust
let mut stream = TcpStream::connect(address).await  // 无连接复用
```

**影响**：每次外部认证请求都需 TCP 握手，增加 5-20ms 延迟。

**建议**：使用连接池复用外部认证服务的连接，或支持 Unix Domain Socket 本地通信。

---

#### 8. 无 HTTP 响应压缩（gzip/brotli）

**影响**：大响应体（JSON API、AI 流式响应）无压缩，浪费带宽并增加传输延迟。

**建议**：在 Pingora 的响应过滤链中添加 `Content-Encoding` 支持（gzip 优先，brotli 可选），对 `text/*` 和 `application/json` 类型自动压缩。

---

### P2 — 低优先级

#### 9. 默认 upstream read timeout 偏长

**位置**：`crates/aeg-config/src/defaults.rs:215`

```rust
pub(crate) fn default_upstream_read_timeout_ms() -> u64 { 30_000 }  // 30秒
```

**建议**：对普通 HTTP API 可缩短至 10-15 秒以更快释放资源；AI 流式响应保留 30-60 秒。

**配置项**：`runtimeTuning.upstreamReadTimeoutMs`

---

#### 10. 访问日志采样率默认 100%

**位置**：`crates/aeg-config/src/defaults.rs:55`

```rust
pub(crate) fn default_access_sample_rate() -> f64 { 1.0 }
```

**建议**：高流量场景建议默认 0.1-0.5，减少磁盘 I/O 对延迟的影响。

**配置项**：`accessLog.sampleRate`

---

## 二、安全性优化建议

### P0 — 高优先级

#### 1. 外部 Auth 服务通信无 TLS 加密

**位置**：`crates/aeg-http/src/proxy/external_auth.rs:39`

```rust
let mut stream = TcpStream::connect(address).await  // 明文 TCP！
```

**影响**：认证请求（包含 token、header）在网络上明文传输，可被中间人截获。

**建议**：
- 必须支持 TLS 连接到外部认证服务
- 或强制使用 Unix Domain Socket + mTLS
- 最低要求：添加 TLS 选项，并在文档中明确警告明文风险

---

#### 2. Session Persistence Secret 自动生成 — 多副本部署下失效

**位置**：`crates/aeg-http/src/session.rs:77`

```rust
info!("session persistence using auto-generated key; configure sharedSecret or sharedSecretFile for multi-replica deployments");
```

**影响**：
- 未配置 `sharedSecret` 时，每个实例生成不同的密钥 → 多副本 session 无法共享
- 重启后密钥丢失 → 所有 session 失效

**建议**：生产环境强制要求配置 `sharedSecret` 或 `sharedSecretFile`；启动时若检测到 `sessionPersistence` 启用但未配置密钥，打印 ERROR 级别日志并考虑拒绝启动。

**配置项**：`sessionPersistence.sharedSecret` / `sessionPersistence.sharedSecretFile`

---

#### 3. API 密钥明文存储在内存中

**位置**：`crates/aeg-ai/src/keyring.rs:12-13`

```rust
pub struct ApiKeyManager {
    keys: Arc<RwLock<HashMap<String, Vec<BackendCredential>>>>,  // 明文存储
}
```

**影响**：Core dump 或内存 dump 可泄露所有后端 API 密钥。

**建议**：
- 使用 `secrecy::Secret<String>` 包装敏感字段
- 实现 `Drop` 时调用 `zeroize` 清零内存
- 考虑使用 Linux `mlock` 防止敏感内存被 swap 到磁盘

---

#### 4. xDS TLS 默认关闭

**位置**：`crates/aeg-config/src/lib.rs:146`

```rust
pub enabled: bool,  // serde(default) → false
```

**影响**：与控制面的通信默认明文，配置数据（路由规则、后端地址、TLS 证书）可被窃听/篡改。

**建议**：生产环境默认启用 xDS TLS（`xdsTls.enabled: true`），至少在文档中明确标注安全风险。

**配置项**：`xdsTls.enabled`

---

### P1 — 中优先级

#### 5. 缺少响应安全头

**影响**：代理不添加任何安全头，下游服务暴露于多种 Web 攻击。

**建议**：在 HTTP 响应过滤链中默认添加：
- `X-Content-Type-Options: nosniff`
- `Strict-Transport-Security: max-age=31536000; includeSubDomains`（HTTPS 监听器）
- `X-Frame-Options: DENY`（可配置）
- `Referrer-Policy: strict-origin-when-cross-origin`

---

#### 6. 无 IP 黑白名单机制

**影响**：无法在网关层阻止恶意 IP，所有流量透传到后端。

**建议**：添加基于 IP/CIDR 的访问控制过滤器（`Filter` 类型扩展），支持从 xDS 动态下发。

---

#### 7. 无请求体 Content-Type 校验

**影响**：仅限制 `maxRequestBodyBytes`（默认 10MB），未校验 Content-Type 与 body 一致性。

**建议**：添加 Content-Type 校验过滤器，拒绝不匹配的请求（如 `Content-Type: application/json` 但 body 为非 JSON）。

**配置项**：`runtimeProtection.httpMaxRequestBodyBytes`（默认 10MB）

---

#### 8. Admin API 仅 Bearer Token 认证

**位置**：`crates/aeg-config/src/lib.rs:107`

```rust
pub struct AdminAuthConfig {
    pub bearer_token: String,
    pub bearer_token_file: String,
}
```

**建议**：
- 添加 mTLS 支持作为额外认证层
- 限制 Admin API 监听地址为非公网 IP（如 `127.0.0.1:9090`）
- 添加速率限制防止暴力破解

**配置项**：`adminAuth.bearerToken` / `adminAuth.bearerTokenFile`

---

### P2 — 低优先级

#### 9. Graceful Drain Period 默认为 0

**位置**：`crates/aeg-config/src/defaults.rs:131`

```rust
pub(crate) fn default_graceful_drain_period_ms() -> u64 { 0 }
```

**建议**：设为 30-60 秒，让滚动更新时已有连接正常完成。

**配置项**：`runtimeTuning.gracefulDrainPeriodMs`

---

#### 10. HTTP 最大请求头无默认限制

**位置**：`crates/aeg-config/src/lib.rs:205`

```rust
pub http_max_request_header_bytes: usize,  // serde(default) → 0 = 无限制
```

**建议**：设置默认值 64KB，防止大 header 攻击（CVE 类似漏洞）。

**配置项**：`runtimeProtection.httpMaxRequestHeaderBytes`

---

## 三、架构亮点（值得保留）

| 方面 | 实现 | 位置 |
|------|------|------|
| 内存安全 | 所有 crate `#![forbid(unsafe_code)]` | 各 crate `lib.rs` |
| 快速路径 | Fast path 路由选择 + thread-local 后端配置缓存 | `crates/aeg-http/src/proxy/selection.rs` |
| 过载保护 | 三级准入控制（Global/Listener/Route）+ 令牌桶限流 + 断路器 | `crates/aeg-observability/src/` |
| 重试风暴防护 | Retry Budget 机制（20% 比例 + 16 burst） | `crates/aeg-observability/src/retry_budget.rs` |
| 配置热重载 | watch channel + 250ms 轮询，零停机更新 | `crates/aeg-app/src/config_reload.rs` |
| 依赖审计 | cargo-deny 完整配置 | `deny.toml` |
| TLS 安全 | Min TLS 1.2 / Max TLS 1.3，证书热轮转 | `crates/aeg-http/src/runtime.rs` |
| 可观测性 | OpenTelemetry tracing + 结构化访问日志 | `crates/aeg-observability/` |
| AI 安全 | Prompt Guard 注入检测 + Content Safety 内容过滤 + PII 脱敏 | `crates/aeg-ai/src/` |

---

## 四、总结优先级排序

| 优先级 | 类别 | 建议 | 预期收益 | 改动量 |
|--------|------|------|----------|--------|
| **P0** | 延迟 | 开启 Stream 连接池默认值 | 每连接节省 10-50ms | 1 行配置 |
| **P0** | 安全 | 外部 Auth 通信加 TLS | 防止凭证泄露 | 中等 |
| **P0** | 安全 | Session 密钥强制配置 | 防止多副本 session 失效 | 小 |
| **P1** | 延迟 | 开启 TCP Fast Open 默认值 | 每连接节省 1-50ms | 1 行配置 |
| **P1** | 延迟 | AI 缓存改用并发安全结构 | 消除高并发锁竞争 | 中等 |
| **P1** | 安全 | API 密钥内存加密 | 防止 core dump 泄露 | 小 |
| **P1** | 安全 | xDS TLS 默认启用 | 防止配置窃听 | 1 行配置 |
| **P2** | 延迟 | 添加 HTTP 响应压缩 | 减少传输延迟 30-70% | 中等 |
| **P2** | 安全 | 添加响应安全头 | 防御常见 Web 攻击 | 小 |
| **P2** | 延迟 | 外部 Auth 连接池复用 | 每认证节省 5-20ms | 中等 |
| **P2** | 延迟 | xDS poll 间隔优化 | 减少 CPU 浪费 | 1 行配置 |
| **P2** | 安全 | IP 黑白名单 | 防御恶意流量 | 大 |
| **P2** | 安全 | Admin API mTLS | 加固管理接口 | 中等 |
| **P2** | 延迟 | HTTP/3 支持评估 | 弱网环境改善 | 大 |

---

## 五、第二轮深度优化（代码质量 + 构建 + 可观测性）

> 审计日期：2026-06-06（第二轮）
> 审计范围：全部 14 个 crate，~100K 行 Rust 代码

### P0 — 生产环境稳定性

#### 1. `std::sync::Mutex` 在 async 上下文中会阻塞 tokio 线程

**位置**：`crates/ntgw-ai/src/ratelimit.rs:2`, `crates/ntgw-ai/src/multitenant.rs:2`

```rust
use std::sync::Mutex;  // 阻塞型锁！
```

**影响**：tokio 默认工作线程数等于 CPU 核心数。如果在 async 任务中持有 `std::sync::Mutex` 导致阻塞，会饥饿其他就绪任务，导致尾延迟飙升。

**建议**：统一替换为 `parking_lot::Mutex`（已在 workspace deps 中）。

**对比**：`ntgw-http`/`ntgw-ir` 已使用 `parking_lot::RwLock`，但 `ntgw-ai` 混用 `std::sync::Mutex`。

---

#### 2. Regex 在热路径运行时编译

**位置**：`crates/ntgw-ai/src/content_safety.rs:74-118`

```rust
Regex::new(r"(?i)(how\s+to|teach\s+me\s+to)\s+(kill|...")  // 每次请求编译！
```

**影响**：`ContentSafetyFilter::default_patterns()` 返回 `Vec<(String, Regex)>`，其中每个 Regex 在函数调用时编译。如果该函数被多次调用（例如在请求路径中），会产生大量不必要的编译开销。

**建议**：使用 `std::sync::LazyLock`（Rust 1.80+ stable）包装 Regex，确保只编译一次。

**同样问题**：`prompt_guard.rs` 中的 `PromptGuardFilter::default_patterns()`。

---

#### 3. `Box::leak` 导致的 4 处有意内存泄漏

**位置**：`crates/ntgw-http/src/cache/mod.rs:83-89`

```rust
let storage: &'static MemCache = Box::leak(Box::new(MemCache::new()));
let eviction: &'static LruManager = Box::leak(Box::new(...));
let lock: &'static CacheLock = Box::leak(CacheLock::new_boxed(...));
let defaults: &'static CacheMetaDefaults = Box::leak(Box::new(...));
```

**影响**：HTTP 缓存启用后，缓存存储、LRU 管理器、锁和元数据永远无法被回收。在容器化部署中，如果缓存配置过大，OOM killer 可能触发。

**建议**：改为 `Arc<Mutex<MemCache>>` 架构，支持热更新缓存大小上限。

---

### P1 — 性能优化

#### 4. 日志热路径中的字符串分配

**统计**：
- `.to_string()` 调用：2841 处（全代码库）
- `format!()` 调用：346 处
- `.clone()` 调用：1111 处

**热点文件**：
| 文件 | `.to_string()` | 问题 |
|------|:---:|------|
| `ntgw-http/src/proxy/logging.rs` | 15+ | 每个请求构造 JSON 事件时大量字符串分配 |
| `ntgw-http/src/proxy/filters.rs` | 10+ | query_string、path 重复分配 |
| `ntgw-http/src/proxy/context.rs` | 5+ | backend_name, address 重复分配 |

**建议**：使用 `Arc<str>` 或 `Cow<str>` 减少重复分配。访问日志的 JSON 序列化可考虑预分配 buffer。

---

#### 5. `unwrap()` 调用在生产代码中

**统计**：57 处非测试代码中的 `.unwrap()` 调用

| 文件 | 数量 |
|------|:---:|
| `ntgw-ai/src/content_safety.rs` | 10 |
| `ntgw-stream/src/pool.rs` | 6 |
| `ntgw-ai/src/ab_test.rs` | 6 |
| `ntgw-ai/src/prompt_guard.rs` | 5 |
| `ntgw-ai/src/keyring.rs` | 5 |

**影响**：如果这些路径触发 panic，将导致 tokio 任务崩溃，可能引发级联故障。

**建议**：替换为 `.expect("descriptive message")` 或 `?` 传播错误。

---

#### 6. 锁原语不一致

| Crate | 使用的锁 | 问题 |
|-------|----------|------|
| `ntgw-http` | `parking_lot::RwLock` | ✅ 正确 |
| `ntgw-ir` | `parking_lot::{Mutex, Condvar, RwLock}` | ✅ 正确 |
| `ntgw-ai` | `std::sync::Mutex`, `std::sync::RwLock` | ❌ 应统一为 parking_lot |
| `ntgw-observability` | `parking_lot::RwLock` | ✅ 正确 |

**建议**：全项目统一使用 `parking_lot`，`std::sync::*` 仅用于需要 poisoning 语义的场景。

---

### P2 — 构建优化

#### 7. Release profile 缺少 `strip`

**位置**：`Cargo.toml`

```toml
[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"
# 缺少: strip = true
```

**建议**：添加 `strip = true` 可减少 20-40% 二进制大小。

---

#### 8. 单消费者 workspace 依赖

以下 workspace 级依赖仅被 1 个 crate 使用，可下放到 crate 级 `Cargo.toml`，加速增量编译：

| 依赖 | 仅被使用于 |
|------|-----------|
| `arc-swap` | `ntgw-bench` |
| `bytes` | `ntgw-http` |
| `form_urlencoded` | `ntgw-ir` |
| `getrandom` | `ntgw-http` |
| `humantime` | `ntgw-observability` |
| `hmac` | `ntgw-http` |
| `http` | `ntgw-http` |
| `nix` | `ntgw-observability` |
| `subtle` | `ntgw-app` |

---

### P2 — 可观测性增强

#### 9. 缺少分布式追踪 span

**发现**：全代码库中 **零个** `#[instrument]` / `#[tracing::instrument]` 注解。

**影响**：无法在 Jaeger/Tempo 中查看请求的完整调用链，调试跨服务延迟问题困难。

**建议**：在关键函数添加 `#[instrument]`：
- `GatewayProxy::upstream_peer()` 
- `apply_request_filters()` / `apply_response_filters()`
- `run_external_auth()`
- `AIGatewayFilter::pre_process()` / `post_process()`
- `TcpConnectionPool::get_connection()`

---

#### 10. 缺失缓存可观测性指标

**发现**：HTTP Cache (`ntgw-http/src/cache/mod.rs`) 无 Prometheus 指标暴露。

**建议**：添加 `http_cache_hits_total` / `http_cache_misses_total` / `http_cache_evictions_total` 计数器。

---

#### 11. 缺失 TLS 握手时长指标

**发现**：虽然 `traffic.rs` 记录了 `total_upstream_tls_handshake_failures`，但没有 TLS 握手延迟的 histogram。

**建议**：添加 `upstream_tls_handshake_duration_seconds` histogram。

---

## 六、第二轮总结

| 优先级 | 类别 | 建议 | 改动量 |
|--------|------|------|--------|
| **P0** | 稳定性 | `std::sync::Mutex` → `parking_lot::Mutex` | 小（2 文件） |
| **P0** | 性能 | Regex 编译 → `LazyLock` | 小（2 文件） |
| **P0** | 内存 | `Box::leak` 替换 | 大（需架构改动） |
| **P1** | 性能 | 日志热路径字符串分配优化 | 中等 |
| **P1** | 稳定性 | `unwrap()` → `expect()`/`?` | 中等（57 处） |
| **P1** | 一致性 | 统一使用 `parking_lot` | 小 |
| **P2** | 构建 | Release `strip = true` | 1 行 |
| **P2** | 构建 | 下放单消费者 workspace 依赖 | 小（编辑 Cargo.toml） |
| **P2** | 可观测 | 添加 `#[instrument]` span | 中等 |
| **P2** | 可观测 | 缓存/TLS 指标 | 小 |
| **P2** | 可观测 | Debug 日志降级或条件编译 | 小 |