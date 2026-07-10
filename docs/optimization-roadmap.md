# Dataplane Optimization Roadmap

日期：2026-06-14

范围：基于 `/root/nantian-gw/dataplane` 的静态代码审查和少量依赖检查整理。未运行全量压测，也未运行 `cargo test --workspace`。性能收益需要用基准测试确认，不应按经验值预估。

## 使用方式

- `[ ]` 表示待处理，`[x]` 表示已处理。
- 建议按 `P0 -> P1 -> P2` 顺序推进。
- 每个条目尽量包含风险、建议、参考文件和验证方式，方便逐项建 issue 或拆 PR。
- 涉及行为变更的项建议先补测试，再改实现。

## P0：优先处理

### P0-1 修正示例配置的安全默认值

- [x] 将 `configs/dataplane/config.yaml` 中的生产风险项改为安全默认值。

风险：

- 代码默认 `httpMaxRequestBodyBytes = 10MiB`、`httpMaxRequestHeaderBytes = 65536`，但示例配置把 body/header/inflight/rate/TCP/UDP 限制都显式设为 `0`。
- 如果用户复制示例配置部署，会绕过代码默认保护。
- 对公网或半公网代理来说，这是最直接的 DoS 风险。

建议：

- `runtimeProtection.httpMaxRequestBodyBytes` 使用 `10485760` 或按业务设定。
- `runtimeProtection.httpMaxRequestHeaderBytes` 使用 `65536` 或更保守值。
- `httpGlobalInflightLimit`、`httpListenerInflightLimit`、`httpRouteInflightLimit` 给非零默认。
- `httpGlobalRateLimitRequestsPerSecond`、`httpListenerRateLimitRequestsPerSecond`、`httpRouteRateLimitRequestsPerSecond` 给保守示例值。
- `tcpGlobalConnectionLimit`、`tcpListenerConnectionLimit`、`udpGlobalDatagramLimit`、`udpListenerDatagramLimit` 给非零示例值。
- 如需开发环境全关闭，单独提供 `config.dev.yaml`，不要让主示例等同于“无保护”。

参考：

- `configs/dataplane/config.yaml:92`
- `crates/ntgw-config/src/defaults.rs:199`

验证：

- 增加/更新配置解析测试，确认缺省值仍为 10MiB/64KiB。
- 增加示例配置测试，确认示例不再把核心保护设为 `0`。
- 运行 `cargo test -p ntgw-config`。

已落地：

- 示例配置已为 HTTP inflight、rate limit、body/header、TCP/UDP admission 设置非零保护值。
- 修复 `RuntimeProtectionConfig::default()`，缺失整个 `runtimeProtection` 段时也会使用 64KiB header limit。
- 新增 bundled config 安全基线测试。

### P0-2 Admin API 非 loopback 绑定时强制鉴权

- [x] 如果 `adminAddr` 不是 loopback 且未配置 bearer token/token file，启动失败。

风险：

- 当前无 token 时中间件直接放行。
- 启动逻辑只输出 warn，不阻止 `0.0.0.0` 暴露 admin API。
- 一旦部署者把 `adminAddr` 改为公网或集群网段监听，风险很高。

建议：

- 解析 `adminAddr`，若 IP 非 loopback 且 `adminAuth.bearerToken` 与 `adminAuth.bearerTokenFile` 都为空，则返回配置错误。
- 如确实需要无鉴权，增加显式开关，例如 `adminAuth.allowUnauthenticated: true`，并在配置和日志中强提示。
- 保留当前 constant-time token 比较逻辑。

参考：

- `crates/ntgw-app/src/admin/auth.rs:68`
- `crates/ntgw-app/src/main.rs:311`
- `configs/dataplane/config.yaml:24`

验证：

- 增加单元测试：`127.0.0.1` 无 token 可启动或仅告警。
- 增加单元测试：`0.0.0.0` 无 token 启动失败。
- 增加单元测试：`0.0.0.0` 有 token 可启动。

已落地：

- 启动阶段在任何 runtime 启动前校验 admin 绑定地址和 bearer auth 状态。
- loopback admin bind 仍允许无 token，但继续输出告警。
- 非 loopback admin bind 无 token 会直接返回配置错误。

### P0-3 AI Gateway 请求体增加硬上限

- [x] 为 AI Gateway 预处理路径增加独立 body 上限，禁止无限整包缓冲。

风险：

- AI 预处理会把完整下游请求体读取到 `Vec<u8>`。
- 如果 `runtimeProtection.httpMaxRequestBodyBytes = 0`，这条路径可以形成内存 DoS。
- 即使全局 body limit 非零，AI 路径也应有更贴近模型/格式的独立上限。

建议：

- 新增配置，例如 `experimental.aiGatewayMaxRequestBodyBytes` 或放入 AI 专属 runtime 配置。
- 默认值建议 1MiB-10MiB，按 AI 请求格式决定。
- 读取 body 时边读边检查，超过即返回 `413`，不要等完整 body 读完。
- 在读取 body 前先按 route/content-type 判断是否需要 AI 处理，避免非 AI 请求进入整包缓冲。
- 中期改造成 streaming-aware 处理，尤其是 SSE、chat completion、multipart 等场景。

参考：

- `crates/ntgw-http/src/proxy/filters.rs:635`
- `crates/ntgw-http/src/proxy.rs:474`

验证：

- 增加 HTTP/1 chunked 超限测试。
- 增加 Content-Length 超限测试。
- 增加 AI 非目标路径不读取 body 的测试。
- 对大 body 请求压测 RSS 峰值。

已落地：

- 新增 `experimental.aiGatewayMaxRequestBodyBytes`，默认 `10485760`。
- 示例配置显式写出 AI Gateway body cap。
- HTTP AI 预处理读取 body 时逐 chunk 检查上限，超过后立即返回 `413`，不再继续聚合。
- 已增加配置默认/覆盖、app 映射和 body limit 边界测试。

后续补强：

- 当前实现仍会在 AI Gateway 启用时读取所有请求体；后续应先按 route/content-type 判定是否需要 AI 处理，再读取 body。
- 仍需补完整 HTTP/1 chunked/content-length 集成测试和 RSS 压测。

### P0-4 HTTP cache miss 改为流式写入或设置响应体缓存上限

- [x] 避免 cache miss 时把完整响应体暂存在 `RequestContext`。

风险：

- 当前 response body filter 会 clone 每个响应 chunk 到 `ctx.cached_response_body`。
- logging 阶段再 drain 写入 cache。
- 大 cacheable response 会造成响应体额外驻留，放大 RSS 峰值。

建议：

- 首选：在 response body filter 中边转发边写 miss handler，避免请求结束后再集中写。
- 如果受 Pingora cache API 限制，至少增加 per-response cache body 上限，超过上限放弃写 cache。
- 对 `Cache-Control`、`Content-Length` 做预判断，大响应直接 skip cache。
- 明确 cache 和 request retry buffer 的内存预算关系。

参考：

- `crates/ntgw-http/src/proxy/context.rs:171`
- `crates/ntgw-http/src/proxy.rs:682`
- `crates/ntgw-http/src/proxy.rs:699`

验证：

- 增加 100MiB cacheable response 的集成测试或压力测试。
- 记录 RSS、分配次数、cache hit/miss 行为。
- 确认 skip cache 时响应仍完整转发。

已落地：

- 新增 `runtimeTuning.httpCache.maxEntrySizeMb`，默认 `16`。
- 示例配置显式写出 `maxEntrySizeMb: 16`。
- cache miss response body 累计超过 per-entry 上限时，会清空已累计 chunks 并放弃本次 cache admission；响应仍继续转发。
- 已增加配置默认/覆盖、cache options 转换和 response body limit 边界测试。

后续补强：

- 当前落地的是内存上限保护，不是完整异步流式写 cache。
- 仍需补大响应集成测试和 RSS/分配数压测。

### P0-5 生产模板启用 xDS TLS/mTLS

- [x] 将生产示例中的 xDS 控制面连接改为 TLS/mTLS，明文只保留 dev 示例。

风险：

- xDS 是控制面信任边界，明文连接容易被中间人篡改路由、backend、证书引用等配置。
- 当前实现支持 TLS/mTLS，但示例配置禁用。

建议：

- 生产示例设置 `xdsTls.enabled: true`。
- 配置 `caPath`、`certPath`、`keyPath`、`domainName`。
- `controlPlaneAddr` 使用 `https://...`。
- 保留 `config.dev.yaml` 使用 `http://127.0.0.1`，避免本地开发复杂化。

参考：

- `configs/dataplane/config.yaml:113`
- `crates/ntgw-xds/src/tls.rs:56`

验证：

- 增加 endpoint normalize 测试：TLS enabled 时 `http://host` 被升级到 `https://host`。
- 增加缺少 cert/key 任一文件时报错测试。
- 集成测试覆盖 mTLS 握手。

已落地：

- 新增 `configs/dataplane/config.production.yaml`，使用 HTTPS 控制面地址。
- 生产模板启用 `xdsTls.enabled: true`，并配置 CA、client cert、client key、domainName 占位路径。
- 生产模板 admin 监听 `0.0.0.0:19080` 时配置 bearer token file。
- bundled config schema 测试已覆盖 local 和 production 模板。

后续补强：

- 仍需补 xDS mTLS 握手集成测试。

## P1：高价值性能优化

### P1-1 扩展 HTTP fast path，减少慢路径分配

- [x] 核心目标已实现：header 全量物化已按快照需求门控，fast path 零 header 分配。

已确认（2026-07-09 复核当前代码）：

- 真正的 fast path（`fast_path_request_from_header`，`proxy/request.rs`）只借用 host/path/method + is_grpc 判断，不构造任何 header map。
- 全量物化由 `Snapshot::request_materialization`（`RequestMaterializationHints`）门控，`proxy.rs` 用 `requires_full_headers()` 决定是否回退慢路径。该 flag 每次快照构建时算一次（`snapshot/helpers.rs::build_request_materialization_hints`），仅当配置真的用到 header 时才为 true：http/grpc route 的 header matcher、session persistence、consistent-hash LB 且 key 为 Header。
- roadmap 原文点名的 CORS / header-modifier / external-auth / Wasm 不经过物化 map，它们在 filter 阶段直接读活 `RequestHeader`；access log 的 `ctx.request_headers` 是另一条独立 lazy 路径（`proxy/request.rs`，仅在 response filter 需要时按 allowlist 填充）。
- 因此 roadmap 建议里“仅在需要时 lazy capture / 区分日志与路由需求”已经落地，且比原设想更细。

剩余更窄的机会（需 profiling 才值得动，非本条主目标）：

- (a) tracing 开启会强制禁用 fast path（`fast_path_request_features_are_safe(request_tracing_enabled, ...)`），但 span 在选路前已用活 header 建好；若确认 tracing 不需要物化 map，放开此限制可让所有开 tracing 的部署每请求少一次全量物化。
- (b) 慢路径确需物化时，`request_headers()` 仍构造 `BTreeMap<String, Vec<String>>`（小写 owned key）；可换更便宜结构，但只帮到已降级的慢路径。

原始现状记录（2026-06-14，已过时）：

- 已有 `HttpFastPathPlan`，架构方向正确。
- 需要访问日志、tracing、完整 headers、source IP 等功能时会回退慢路径。
- 慢路径会构造 `RequestMeta`、`BTreeMap<String, Vec<String>>` 和多个 `String`。

建议：

- 对只需要 method/path/host/content-length 的限流和路由选择使用 `RequestView`，避免完整 header materialization。
- `request_headers` 仅在 CORS、header modifier、external auth、Wasm、access log 等确实需要时 lazy capture。
- 区分“日志需要字段”和“路由选择需要字段”，避免为了日志破坏 fast path。
- 对 header names/常见 headers 使用 borrowed view 或小型结构，不在每个请求中转小写字符串。

参考：

- `crates/ntgw-ir/src/http_fast_path.rs`
- `crates/ntgw-http/src/proxy/request.rs:206`
- `crates/ntgw-http/src/proxy/context.rs:129`

验证：

- 增加 Criterion benchmark：route selection、request filter、fast path with/without access log。
- 用 `heaptrack`、`dhatu` 或 jemalloc stats 观察 alloc count。

### P1-2 Traffic latency histogram 从线性 Vec 改为 Map

- [x] 已实现（2026-07-09）：histogram 存储从线性 `Vec` 改为 `hashbrown::HashTable`，查找 O(1) 且热路径零分配。

已落地：

- `TrafficState.request_latency_ms_histograms` 现为 `HashTable<(TrafficLatencyLabels, TrafficHistogramState)>`；`observe`/`merge` 用 `HashTable::entry(hash, eq, hasher)`，命中零分配，未命中才构造 owned key。snapshot 阶段仍 `sort_by(labels)`，对外 `TrafficSnapshot` 仍是排序 `Vec`，语义/输出不变。
- 新增高基数 benchmark（`ntgw-bench` 场景 `traffic_observe_high_cardinality`，1024 series warmup 后稳态 lookup）：p99 ~1µs 且不随基数增长，2000 次 observe 热路径 0 分配。

原始建议（2026-06-14）：

风险：

- `TrafficState.request_latency_ms_histograms` 当前是 `Vec<(TrafficLatencyLabels, TrafficHistogramState)>`。
- 每次写入需要按标签组合线性查找。
- listener/route/status/response_flag 基数升高后，指标写入路径会变成热点。

建议：

- 改为 `HashMap<TrafficLatencyLabels, TrafficHistogramState>`。
- 如果需要稳定输出顺序，snapshot/render 阶段再排序。
- 对 `response_flag`、listener、route kind 做基数上限或归一化。
- 保持 shard 设计，避免回退到全局锁。

参考：

- `crates/ntgw-observability/src/traffic/stats.incl.rs:17`
- `crates/ntgw-observability/src/traffic/stats.incl.rs:149`
- `crates/ntgw-observability/src/traffic/stats.incl.rs:513`

验证：

- 增加高基数指标写入 benchmark。
- 比较 10、100、1000、10000 标签组合下的 observe latency。
- 确认 Prometheus 输出不发生语义回归。

### P1-3 Wasm hook 引入实例池或预实例化池

- [ ] 避免每次 hook 调用都新建 `Store` 并 instantiate module。

Baseline（2026-07-10，`ntgw-bench` 新增 `wasm_hook_empty_invoke` / `wasm_hook_header_heavy_invoke`，commit `fc144ff`，release，2000 iters/warm-up 后测量）：

- empty hook（无 header）：avg 7.38µs、p50 5.95µs、p99 18.32µs，**20 allocs/call**、7202 bytes/call。这是纯 fresh-`Store` + `InstancePre::instantiate` 的每请求开销（guest 只 `i32.const 0`，不含执行）。
- header-heavy（16 header）：avg 8.77µs、p50 8.16µs、p99 17.48µs，**53 allocs/call**、9186 bytes/call。
- 两个独立信号：(a) instantiate 本身 ~7.4µs + 20 allocs/call，是 pooling allocator 能打的部分；(b) header marshaling 每 16 header 多 ~33 allocs/call、~1.4µs，是 allowlist/lazy hostcall 能打的部分，改动更小、无虚拟内存预留风险。
- 判断：开销真实但**仅在配置了 wasm 插件时**才在请求路径上；无插件部署此项为零。pooling allocator 属高风险生产改动（`InstanceAllocationStrategy::Pooling` 会按上限预留虚拟内存，配置不当会让 instantiate 直接失败），未在无实测收益目标前擅自上。下一步取舍见下方"实施选项"。

实施选项（择一，待定）：

- **A. 先做 header marshaling(低风险)**：把 `HashMap<String,String>` 全量 clone 改 allowlist / lazy hostcall lookup，只动 `ntgw-wasm` + 调用点，无虚拟内存风险；预期削掉 header-heavy 那 ~33 allocs/call。
- **B. pooling allocator(高风险高收益)**：engine 启用 `Pooling`，保守配置上限 + 失败回退，预期削掉 instantiate 的 per-call alloc/syscall；需专门的上限压测防 instantiate 失败。
- **C. 以 baseline 结项**：若判定 7.4µs/20-alloc 对目标部署可忽略，用数据把 P1-3 关成 won't-fix，而非凭感觉。

风险：

- 当前每个 hook 都 fresh `Store`，再 instantiate module。
- 这对隔离友好，但高频请求下 CPU 成本较高。
- header 会 clone 成 `HashMap<String, String>`，也增加分配。

建议：

- 为每个 plugin/hook/worker 建 bounded instance pool。
- 每次调用前重置 host context，调用后清理状态再归还池。
- 对无法安全复用的 plugin 保留当前 fresh instantiate 模式。
- header 改为 allowlist 或 hostcall lazy lookup，避免全量 clone。

参考：

- `crates/ntgw-wasm/src/plugin.rs:164`
- `crates/ntgw-wasm/src/plugin.rs:189`
- `crates/ntgw-http/src/proxy.rs:654`

验证：

- benchmark：empty hook、header lookup hook、body processing hook。
- 对比 fresh instantiate、pooled instance、disabled wasm 三种模式。
- 验证 plugin 状态不会跨请求泄漏。

### P1-4 Wasm sandbox 补齐 table/module/source 限制

- [x] 补齐 table growth、module 大小上限，关闭未用 wasmtime feature（2026-07-09）。

已落地：

- `ResourceLimiter::table_growing` 从无条件 `Ok(true)` 改为按 `PluginContext::table_elements_limit` 门控；`invoke_hook` 用 `engine::MAX_WASM_TABLE_ELEMENTS`（10_000）设定。
- 新增 `engine::MAX_WASM_MODULE_BYTES`（32 MiB），在三处 load 路径（`load_plugin`、`load_or_update`、`AISandbox::load_module`）编译前检查，超限返回 `LoadFailed`。
- `global_engine_config` 关闭 `wasm_component_model` 和 `wasm_multi_memory`：加载器只用核心 `Module`、host 只取单个 `"memory"` export，关掉纯减攻击面/编译成本，不影响现有插件（集成测试 `test_load_and_invoke_plugin` 仍通过）。
- 新增单测：table/memory limiter 门控、oversized module 拒绝。

已确认无需再做（roadmap 已过时）：

- epoch 线程**已有** stop：`epoch_running: AtomicBool` + `shutdown()` + `Drop for PluginManager`（join）。

范围外/后续：

- table/module 上限目前是 crate 级常量（安全下限），未接入控制面 per-plugin 配置——接入需改 IR + BSR proto，另立项。
- 编译 timeout / 离线预编译未做（当前已有 SHA 命名的序列化缓存）。
- `AISandbox` 的 tokenize/embed 路径未挂 `store.limiter(...)`（memory/table 限制在该路径未生效），属独立缺口。

建议：

- `ResourceLimiter::table_growing` 增加 `max_table_elements`。
- 加 `max_wasm_module_bytes`，load plugin 前先检查。
- 对编译/加载增加 timeout 或离线预编译策略。
- epoch worker 增加 shutdown 标记或由 runtime 托管。
- 关闭不必要的 Wasmtime feature，例如只在确实需要时启用 component model/multi memory。

参考：

- `crates/ntgw-wasm/src/engine.rs:25`
- `crates/ntgw-wasm/src/engine.rs:35`
- `crates/ntgw-wasm/src/plugin.rs:97`

验证：

- 构造 table growth 恶意模块，确认被拒绝。
- 构造超大 module，确认 load 前拒绝。
- 插件卸载/重载测试中确认后台线程不泄漏。

### P1-5 TCP upstream pool 降低热点 backend 竞争

- [x] 已加多线程竞争 benchmark 并收窄锁持有（2026-07-09）；worker-sharded 重构暂缓，按数据决定是否需要。

已落地：

- 新增多线程 pool 竞争 benchmark（`crates/ntgw-stream/src/bench.rs` 的 `TcpPoolContentionFixture`，`ntgw-bench` 场景 `stream_pool_contention_hot_key` / `stream_pool_contention_spread`）。单线程 harness 内起 N 个 OS 线程打预热过的池，hot_key（同 backend，单 DashMap 分片）对 spread（每 worker 各自 backend）做对照。
- `get_connection` 不再在持 DashMap 分片锁时跑 `try_read`：改为锁内 O(1) `pop` 候选、锁外做超时判断和存活探测；LIFO、命中即返回、剩余留池、淘汰规则不变。
- 效果（8 worker × 4000 ops，全命中复用）：同 backend 竞争惩罚（hot_key vs spread p99 比）从约 7× 收窄到约 3.4×，hot_key p99 约 0.081ms → 0.032ms。

后续可选（需 profiling 支撑再做）：

- worker-sharded 本地池 / 每 backend 多 shard queue（roadmap 原建议），改动大、涉及连接亲和与 draining。
- eviction 计数指标（当前只 `debug!` 未计数）。

原始风险记录：

- 当前每个 backend key 下一个 `Vec<IdleConnection>`。
- 同一 backend 高并发时，单 key entry 可能成为锁竞争点。

建议：

- 按 worker shard 建本地池，优先复用本 worker 的连接。
- 或每 backend 使用多个 shard queue。
- 池大小按 backend 分配，避免少数 backend 吞掉全部 idle budget。
- 暴露 pool hit/miss/eviction 指标，辅助容量调优。

参考：

- `crates/ntgw-stream/src/pool.rs:12`
- `crates/ntgw-stream/src/pool.rs:49`
- `crates/ntgw-stream/src/pool.rs:83`

验证：

- benchmark：单 backend 高并发、多 backend 分散并发、连接频繁 idle timeout。
- 观察 p99 connect latency、pool hit ratio、lock contention。

### P1-6 小热路径去分配

- [x] 将 cache hit 方法判断中的 `to_uppercase()` 改为 `eq_ignore_ascii_case`。

风险：

- 当前每次 cache hit 判断会分配 uppercase string。
- 单次收益小，但路径足够热，且修改风险低。

建议：

- 替换为：

```rust
if !ctx.method.eq_ignore_ascii_case("GET") && !ctx.method.eq_ignore_ascii_case("HEAD") {
    return Ok(false);
}
```

参考：

- `crates/ntgw-http/src/proxy/filters.rs:700`

验证：

- 增加 GET/head/post 大小写测试。
- 运行 `cargo test -p ntgw-http`。

已落地：

- 新增 `cache_lookup_method_allowed()` helper，使用 `eq_ignore_ascii_case()` 判断 GET/HEAD。
- `try_cache_hit()` 不再为方法判断分配 uppercase `String`。
- 已增加 GET/head/post/empty 方法边界测试。

## P1：安全加固

### P1-7 限制 access log route annotation 覆盖文件路径

- [x] 禁止或限制通过 route annotation 覆盖 access log path。

风险：

- route annotation 可以覆盖 access log `path`。
- 如果 route annotation 来自控制面或租户输入，等价于允许远端配置写本地路径。

建议：

- 默认禁止 `path` annotation，只允许 `enabled`、`mode`、`sample-rate`、`format` 等低风险字段。
- 如必须允许 path，只允许 `stdout`、`stderr` 或指定目录下的相对文件名。
- 拒绝绝对路径、`..`、设备文件、软链接穿越。

参考：

- `crates/ntgw-observability/src/access.rs:340`
- `crates/ntgw-http/src/proxy/logging.rs:145`

验证：

- 增加 annotation path 被忽略或拒绝测试。
- 确认 `enabled`、`mode`、`sample-rate`、`format` 等低风险字段仍可覆盖。

已落地：

- `access-log-path` route annotation 现在会被忽略，不再能覆盖 dataplane 本地 access log 输出路径。
- 保留 `enabled`、`mode`、`sample-rate`、`format` route annotation 覆盖能力。
- 已更新 route override 单元测试，覆盖 path annotation 被忽略且其它字段仍生效。

### P1-8 精简生产 Docker runtime 镜像

- [x] 主运行镜像已精简为仅 `ca-certificates` + `ntgw-app`，且非 root 运行（2026-07-10 复核）。

已确认（2026-07-10 复核当前代码）：

- runtime 阶段（`Dockerfile:39-49`）只 `apt-get install ca-certificates`，roadmap 点名的 `curl`/`dnsutils`/`iproute2`/`netcat-openbsd`/`procps`/`tcpdump` 均已不在镜像内。
- `USER 65532`（`Dockerfile:47`）非 root 运行，roadmap 建议的“设置非 root 用户”已达成。
- 核心攻击面收敛诉求已满足。

范围外/后续（非本条主目标）：

- 未提供独立 `Dockerfile.debug` / build-arg debug variant——当前若需排障需临时装工具或用别的镜像；若确有需要可另立项。
- 只读 rootfs、drop capabilities、seccomp/apparmor 属部署时的 runtime 加固，不在 Dockerfile 范围内。

风险：

- 当前 runtime image 安装 `curl`、`dnsutils`、`iproute2`、`netcat-openbsd`、`procps`、`tcpdump`。
- 这些工具便于排障，但会扩大容器逃逸或横向移动后的可用工具面。

建议：

- 主镜像只保留 `ca-certificates` 和 `ntgw-app`。
- 新增 `Dockerfile.debug` 或 build arg 生成 debug variant。
- 设置非 root 用户运行。
- 运行环境建议只读 rootfs、drop capabilities、seccomp/apparmor。

参考：

- `Dockerfile:41`

验证：

- 主镜像启动冒烟测试。
- debug 镜像仍包含排障工具。
- 容器扫描确认高危包减少。

### P1-9 CI/release 使用固定 Rust 1.96.0

- [ ] 将 GitHub Actions 的 `toolchain: stable` 改为固定工具链或读取 `rust-toolchain.toml`。

风险：

- 文档和 Docker 使用 Rust 1.96.0。
- CI/release 使用 stable，可能与本地和 Docker 构建不一致。
- 编译器差异可能造成 clippy、fmt、依赖 MSRV 或性能行为差异。

建议：

- `dtolnay/rust-toolchain` 使用 `toolchain: 1.96.0`。
- 或不显式设置 toolchain，让 action 读取 `rust-toolchain.toml`。
- release workflow 同步修改。

参考：

- `.github/workflows/ci.yml:21`
- `.github/workflows/release.yml:18`
- `Dockerfile:1`

验证：

- CI 全部 job 通过。
- release workflow dry-run 或手工触发验证。

### P1-10 将供应链检查纳入 CI

- [x] 增加 `cargo deny check` 到 CI（2026-07-10 复核已落地）。

已确认（2026-07-10 复核当前代码）：

- `.github/workflows/ci.yml` 已有独立 `cargo-deny` job（`ci.yml:63`），安装 `cargo-deny --locked` 后运行 `cargo deny check advisories bans licenses sources`，四项全覆盖。
- 本轮 RUSTSEC-2026-0204 正是被此 job 拦下，验证其确实生效。

现状：

- 本地运行 `cargo deny check` 结果通过：`advisories ok, bans ok, licenses ok, sources ok`。
- 输出提示大量 duplicate crate，需要逐步治理。
- `cargo audit` 因 registry yanked 检查返回 `403/timeframe` 未完整成功，但报告了已知 `RUSTSEC-2024-0437` protobuf 2.x，经 `prometheus -> pingora` 引入。
- `deny.toml` 已对该风险做例外说明。

建议：

- CI 增加 `cargo deny check`。
- 对 `cargo audit` 可配置只做 advisory 检查，或接受网络不稳定导致的非强制 job。
- 发布容器时增加 Trivy/Grype 扫描和 SBOM。
- 跟踪 Pingora/prometheus/protobuf 升级窗口。

参考：

- `deny.toml`
- `.github/workflows/ci.yml`

验证：

- CI 新 job 通过。
- intentionally 引入 forbidden license/advisory 时 job 能失败。

## P2：中期治理

### P2-1 迁移 `serde_yaml`

- [ ] 评估从 `serde_yaml 0.9.34+deprecated` 迁移。

风险：

- `serde_yaml` 已 deprecated。
- 当前由 `ntgw-config` 直接使用，同时 Pingora 依赖链也会引入。

建议：

- 短期限制配置文件来源和大小，不把 YAML 解析暴露为远程输入。
- 中期评估 `serde_yml`、TOML、JSON 或更明确 schema 的配置格式。
- 若保留 YAML，隔离解析层并加强错误上下文和 schema 校验。

参考：

- `Cargo.lock`
- `crates/ntgw-config`

验证：

- 配置兼容性测试。
- 模糊测试 YAML 配置解析。

### P2-2 建立性能基准和回归门槛

- [ ] 为核心热路径补 benchmark，并在优化 PR 中使用数据决策。

建议覆盖：

- `ntgw-ir` route selection。
- HTTP fast path 和慢路径 request filter。
- cache hit/miss。
- traffic stats observe 高基数写入。
- Wasm hook 调用。
- TCP/UDP proxy 热路径。
- AI gateway body parse 和 pre/post process。

建议指标：

- throughput。
- p50/p95/p99 latency。
- RSS 峰值。
- allocation count/bytes。
- CPU cycles 或 flamegraph。
- lock contention。

验证：

- 新增 `cargo bench` 或 `ntgw-bench` 场景。
- 对核心 benchmark 记录 baseline。
- PR 中附优化前后数据。

### P2-3 运行镜像和发布链路硬化

- [ ] 增加发布产物的 SBOM、签名和镜像扫描。

建议：

- 使用 `syft` 生成 SBOM。
- 使用 `cosign` 签名镜像和 release binary。
- 使用 Trivy/Grype 扫描镜像。
- release note 中记录 Rust toolchain、git SHA、Cargo.lock hash、base image digest。

验证：

- release workflow 生成 SBOM。
- 镜像 digest 可复现追踪。

## 已确认的正向设计

- 多个 crate 使用 `#![forbid(unsafe_code)]`，不要破坏这个边界。
- `ntgw-proto` 使用 vendored protoc，不依赖系统 `protoc`。
- release profile 使用 `thin LTO`、`codegen-units = 1`、`panic = abort`，适合生产二进制。
- TLS asset materialization 使用原子写入、`0600` 文件、`0700` 目录，设计较稳。
- backend TLS validation 支持 hostname/custom SAN 校验，方向正确。
- HTTP fast path 已存在，后续优化应在此基础上扩展，而不是另起一套路由逻辑。

## 本轮已执行的轻量检查

- `cargo deny check`
  - 结果：通过。
  - 输出摘要：`advisories ok, bans ok, licenses ok, sources ok`。
  - 注意：有大量 duplicate crate warning，建议后续治理。

- `cargo audit`
  - 结果：未完整成功。
  - 原因：crates.io yanked 检查多次返回 `403 Forbidden` 或 timeout。
  - 仍报告：`RUSTSEC-2024-0437`，`protobuf 2.28.0` 经 `prometheus 0.13.4 -> pingora` 引入。
  - 备注：该风险已在 `deny.toml` 中有例外说明。

## 推荐拆分顺序

1. `security/config-defaults`: 修示例配置保护项，补配置测试。
2. `security/admin-auth`: 非 loopback admin 强制 token，补启动校验测试。
3. `security/xds-tls-template`: 拆 dev/prod 配置模板，生产启用 xDS TLS/mTLS。
4. `perf/ai-body-limit`: AI Gateway body 硬上限和超限测试。
5. `perf/cache-streaming-write`: cache miss 流式写入或 response cache body 上限。
6. `perf/traffic-histogram-map`: traffic latency histogram 改 HashMap，补高基数 benchmark。
7. `perf/wasm-instance-pool`: Wasm hook 实例池和 header lazy lookup。
8. `security/access-log-path`: 限制 route annotation 覆盖 log path。
9. `ci/pinned-toolchain-deny`: CI 固定 Rust 1.96.0，加入 `cargo deny check`。
10. `container/minimal-runtime`: 精简 runtime 镜像，新增 debug 镜像。
