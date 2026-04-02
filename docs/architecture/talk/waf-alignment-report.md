# Cloudflare WAF 规则实现与架构对齐检查报告

> 注：本文记录的是修复前的对齐检查结果。当前实现已切换为 active 对象 allowlist，前后端显示与后端同步已经统一。

## 结论

当前实现不是严格的一对一映射白名单，而是“稳定命名空间 + 编号位数 + 后缀白名单”的结构化规则。它能拦截明显越界请求，但不能证明某个请求对应的是一个已存在的具体对象。

如果验收标准是“只放行已知存在的单个对象”，那么现状不满足。如果验收标准是“只允许 img/public 前缀下符合编号和扩展名规范的请求”，那么当前实现与现有架构文档基本一致。

## 实际实现状态

- 对象 Key 由编号分配器生成，格式固定为 img/public/{number}.{ext}。
- WAF 规则由编号位数和后缀列表拼成正则，再通过 starts_with + not matches 的组合表达为 Cloudflare 规则。
- 删除回收流程会同步 WAF 规则，但同步源是 tracked extensions 和桶统计，不是逐对象 inventory。
- 前端插件页展示的是 pattern、suffix buckets 和范围信息，不是每个对象的精确清单。

这意味着当前系统实现的是“命名空间门禁”，不是“对象真实性校验”。

## 与架构文档的对比

架构文档当前已经明确了以下方向：

- 防护主轴是 WAF 正则白名单 + 命名规范 + 范围限制。
- 对象 Key 契约强调稳定命名空间、顺序编码和禁止时间分片。
- 映射表策略默认关闭，不参与主键生成。
- 参数层只保留了 WAF 更新策略、编号位数上限等待定项，没有定义“逐对象白名单”或“对象级 allowlist”机制。

因此，从“当前规划”的角度看，现有实现并没有偏离文档；相反，它是在按文档里已经冻结的结构化白名单思路落地。

## 风险判断

1. 现有规则可以阻止明显的非法路径和错误后缀，但无法阻止允许命名空间内的枚举尝试。
2. 只要路径满足位数和后缀白名单，就会被视为合法候选，即使该对象并不存在。
3. 所以它更适合做边缘层范围约束，而不是做一映射级别的防穿透保证。

## 结论分级

### 对当前架构是否合理

合理。当前代码、前端展示和架构文档在同一条线上：都采用结构化正则白名单，而不是对象逐条登记。

### 对严格一映射要求是否达标

不达标。当前设计没有提供对象级 allowlist，因此不能把它当作“每个有效对象一条白名单”的实现。

## 建议

- 如果目标仍然是降低无效回源成本，当前方案可以保留。
- 如果目标升级为严格防穿透，应先重新定义架构边界，再考虑对象级 allowlist、签名访问或服务端鉴权之一。
- 在未重定义需求前，不建议把这件事视为代码缺陷，更准确地说是需求强度高于当前架构定义。

## 证据索引

- [src-tauri/src/storage/key_allocator.rs](../../../src-tauri/src/storage/key_allocator.rs#L118-L123)
- [src-tauri/src/runtime/adapter_runtime.rs](../../../src-tauri/src/runtime/adapter_runtime.rs#L212-L245)
- [src-tauri/src/runtime/adapter_runtime.rs](../../../src-tauri/src/runtime/adapter_runtime.rs#L545-L565)
- [apps/desktop/src/utils/waf.ts](../../../apps/desktop/src/utils/waf.ts#L81-L92)
- [apps/desktop/src/pages/PluginsPage.vue](../../../apps/desktop/src/pages/PluginsPage.vue#L62-L74)
- [docs/architecture/architecture.md](../architecture.md#L370)
- [docs/architecture/architecture.md](../architecture.md#L568)
- [docs/architecture/talk/颗粒度对齐报告-v1.5.md](./颗粒度对齐报告-v1.5.md#L42-L52)
- [docs/architecture/talk/颗粒度对齐报告-v1.5.md](./颗粒度对齐报告-v1.5.md#L86-L95)
- [docs/architecture/talk/颗粒度对齐报告-v1.5.md](./颗粒度对齐报告-v1.5.md#L121-L124)
- [docs/architecture/talk/颗粒度对齐报告-v1.5.md](./颗粒度对齐报告-v1.5.md#L434-L435)