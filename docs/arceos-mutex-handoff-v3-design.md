# ArceOS RawMutex 第三版 Handoff 状态机设计

## 1. 目的

本文档给出 ArceOS `RawMutex` 的第三版 handoff 状态机设计，目标是：

- 保留 handoff 的公平性和性能方向
- 修复当前 handoff 在多核高并发场景下的协议不闭合问题
- 明确状态机边界，避免继续在局部逻辑上打补丁

这份设计文档是后续实现、评审和回归验证的基线，不要求在一轮修改中全部完成。

---

## 2. 背景问题

当前 `RawMutex` 的 handoff 机制已经证明存在问题：

- `unlock()` 直接指定 waiter 为下一任 owner
- waiter 被唤醒、被调度，甚至从 `lock()` 返回之后
- 系统整体仍可能停在“逻辑 owner 存在但无前进”的状态

因此问题不再是：

- waiter 没被唤醒
- waiter 没被调度
- waiter 没看到自己是 owner

而是：

> handoff 只完成了局部语义上的所有权宣布，没有形成全局可收敛的接管协议。

---

## 3. 设计目标

第三版状态机需要满足以下目标：

1. **单一真相**
   锁状态必须由一个统一的原子状态字段表达，避免多个原子变量拼协议。

2. **显式阶段**
   必须区分：
   - 锁空闲
   - 锁被某个 owner 持有
   - handoff 正在等待目标 waiter 接管

3. **显式确认**
   waiter 必须显式确认 handoff 接手，而不是仅凭 `owner_id` 被改成自己就默认完成。

4. **安全回退**
   如果 handoff 迟迟没有完成，系统必须能安全退回到普通竞争模式，而不会形成“幽灵 owner”。

5. **多核可推理**
   在多核下，任何线程都只能根据统一状态做判断，避免“局部看成功，全局不收敛”。

6. **可回归验证**
   状态机必须方便用现有 `concurrency_stress` 测例回归。

---

## 4. 总体思路

第三版放弃“多个原子变量共同描述 handoff 语义”的方式，改为：

- 用一个 `AtomicU64 state` 表示锁的完整状态
- 所有 handoff、accept、fallback 都通过对这个统一状态的 CAS 完成

核心思想是：

> 只有 waiter 自己通过 CAS 将状态从 `HandoffPending(target)` 改成 `Owned(target)`，handoff 才算真正完成。

---

## 5. 状态定义

建议使用三态：

1. `Unlocked(epoch)`
2. `Owned(owner, epoch)`
3. `HandoffPending(target, epoch)`

说明：

- `epoch` 是代际号（版本号）
- 每次 `unlock` 或 `fallback` 时递增
- waiter 接手时必须匹配当前 epoch

### 5.1 为什么要有 epoch

引入 `epoch` 的目的：

- 防止陈旧 waiter 误接手旧的 handoff
- 防止 fallback 后又被旧 handoff 路径重新影响
- 减少 ABA 风险

也就是说：

- `task_id` 说明“交给谁”
- `epoch` 说明“是哪一轮交接”

---

## 6. 状态编码建议

建议把 `state` 编码为：

```text
[ tag | epoch | task_id ]
```

其中：

- `tag`：高位，表示状态种类
- `epoch`：中间位，表示版本
- `task_id`：低位，表示 owner 或 handoff target

### 6.1 逻辑编码示意

```rust
enum Tag {
    Unlocked = 0,
    Owned = 1,
    HandoffPending = 2,
}
```

示意辅助函数：

```rust
fn encode_unlocked(epoch: u64) -> u64
fn encode_owned(owner: u64, epoch: u64) -> u64
fn encode_handoff(target: u64, epoch: u64) -> u64

fn tag(state: u64) -> Tag
fn epoch(state: u64) -> u64
fn task_id(state: u64) -> u64
```

---

## 7. lock() 协议

`lock()` 的主路径分为三种：

### 7.1 锁空闲

如果看到：

```text
Unlocked(epoch)
```

则尝试：

```text
CAS(Unlocked(epoch) -> Owned(self, epoch))
```

成功则拿锁成功。

### 7.2 handoff 指定给自己

如果看到：

```text
HandoffPending(self, epoch)
```

则尝试：

```text
CAS(HandoffPending(self, epoch) -> Owned(self, epoch))
```

成功则表示：

- waiter 显式接受 handoff
- handoff 正式完成

### 7.3 其他情况

如果看到：

- `Owned(other, epoch)`
- `HandoffPending(other, epoch)`

则进入等待。

### 7.4 wait 条件

等待条件统一基于单状态判断：

```rust
wait_until(|| {
    let s = state.load(Acquire);
    is_unlocked(s) || is_handoff_to_me(s, self_id)
})
```

不要再依赖多个原子字段的组合判断。

---

## 8. try_lock() 协议

`try_lock()` 也遵循相同状态机：

1. `Unlocked(epoch)` -> `Owned(self, epoch)` 直接 CAS
2. `HandoffPending(self, epoch)` -> `Owned(self, epoch)` 直接 CAS
3. 其他状态直接失败

这样 `try_lock()` 与 `lock()` 语义保持一致。

---

## 9. unlock() 协议

### 9.1 无 waiter

当前状态：

```text
Owned(self, epoch)
```

若无 waiter：

```text
store(Unlocked(epoch + 1))
```

### 9.2 有 waiter

当前状态：

```text
Owned(self, epoch)
```

步骤：

1. 从等待队列中选择 waiter `T2`
2. 尝试：

```text
CAS(Owned(self, epoch) -> HandoffPending(T2, epoch + 1))
```

3. 唤醒 `T2`
4. 等待有限轮次，观察 `T2` 是否把状态改成：

```text
Owned(T2, epoch + 1)
```

若成功，则 handoff 完成。

### 9.3 fallback

如果经过有限轮次后状态仍然是：

```text
HandoffPending(T2, epoch + 1)
```

则尝试：

```text
CAS(HandoffPending(T2, epoch + 1) -> Unlocked(epoch + 1))
```

成功后：

- 退回普通竞争模式
- 可以再 `notify_one()` 一次，避免 waiter 丢机会

---

## 10. 为什么这版比前几版更强

### 10.1 不再把 handoff 当作“已完成状态”

旧设计的问题是：

- 一旦指定 waiter，就等价于逻辑 owner 已改变

第三版里：

- `HandoffPending` 只是“交接提议”
- `Owned(target)` 才表示“接手已完成”

### 10.2 所有线程看到的是统一状态

旧版本中，多个原子字段可能导致：

- 本地观察看似合理
- 全局却不一致

第三版中：

- 所有判断都只基于 `state`
- 更容易推理

### 10.3 epoch 避免陈旧 handoff

旧 waiter 在未来某个时刻即使被调度，也不会把旧 handoff 当成当前 handoff。

---

## 11. 如何避免新竞态

这版设计虽然更强，但必须遵守以下原则：

1. **所有状态流转都通过 CAS**
   不能用多个 `store` 拼成一个语义操作。

2. **只用一个原子字段描述协议状态**
   避免再出现 `owner_id + handoff_to` 这种拼接式协议。

3. **waiter accept 时必须匹配 `(tag, target, epoch)`**
   不能只匹配 `task_id`。

4. **fallback 时必须匹配同一 `(target, epoch)`**
   否则不能回退。

5. **状态机中不允许“隐式接手”**
   只有 waiter 自己把 `HandoffPending(self, e)` CAS 成 `Owned(self, e)` 才算接手成功。

---

## 12. 建议实现步骤

1. 把 `RawMutex` 字段改成：

```rust
state: AtomicU64
```

2. 实现编码/解码辅助函数
3. 重写：
   - `lock()`
   - `try_lock()`
   - `unlock()`
   - `is_locked()`
4. 保留最少量状态日志（仅开发阶段）
5. 用 `concurrency_stress` 回归
6. 再跑 `axsync` 自带单测

---

## 13. 回归验证重点

回归时重点观察：

1. 是否还能出现：
   - `before_lock` 大量堆积
   - `in_mutex = 0`
   - `progress` 长时间停滞

2. 是否还能出现：
   - `release` 已推进
   - 但系统整体无前进

3. 如果仍有问题，重点检查：
   - fallback 是否正确触发
   - epoch 是否避免了陈旧 waiter 接手

---

## 14. 当前建议结论

第三版状态机的价值在于：

- 它不是在旧 handoff 上继续打补丁
- 而是把 handoff 定义成一个明确、可推理、可回退的协议

如果后续仍要保留 handoff，这应当是下一步实现的基础版本。

---

## 15. 当前实现落地

截至当前实现，第三版 handoff 不再只是文档设计，而是已经在代码中按如下方式落地：

1. `RawMutex` 使用单一 `AtomicU64 state`
2. 状态仍为三态：
   - `Unlocked(epoch)`
   - `Owned(owner, epoch)`
   - `HandoffPending(target, epoch)`
3. waiter 只能通过：

```text
CAS(HandoffPending(self, epoch) -> Owned(self, epoch))
```

显式 accept handoff

4. `unlock()` 发起 handoff 后，不立即认为交接完成，而是：
   - 先进入 `HandoffPending(target, next_epoch)`
   - 给予目标 waiter 一个有限的 accept 窗口
   - 若窗口内未完成 accept，再 fallback 到 `Unlocked(next_epoch)`

当前 accept 窗口的实现策略是：

- 先短暂自旋
- 再主动 `yield_now()` 若干次
- 最后才 fallback

也就是说，当前版本不是“纯 handoff”实现，而是：

> 两阶段 handoff + 有限确认窗口 + 安全 fallback

---

## 16. 与调度的配套要求

这次实现过程中，一个重要结论是：

> 两阶段 handoff 本身是必要的，但并不足够；若目标 waiter 在远端 CPU，还需要跨 CPU 的调度推进。

更具体地说：

- 本地 `yield_now()` 只能立即影响当前 CPU 的 run queue
- 如果 handoff target 被唤醒到远端 CPU，本地 owner 的 `yield_now()` 并不能直接让远端 target 及时运行
- 因此，在 SMP 下，handoff 想真正高比例生效，还需要远端 `resched` 能力

当前在 `x86_64` 路径上的配套实现是：

1. 当 `WaitQueue::notify_one_with(..., resched = true)` 唤醒的任务位于当前 CPU：
   - 仍沿用已有 `preempt_pending` 路径

2. 当被唤醒任务位于远端 CPU：
   - 通过 IPI 在目标 CPU 上触发一次远端 reschedule 提示

这条能力目前仅在 `x86_64` 回归验证中启用。

对于 `aarch64 + axplat-dyn` 路径，当前仍缺失平台层 `send_ipi()` 支持，因此该配套机制尚未完成闭环。

---

## 17. 已验证结论

### 17.1 原始单阶段 handoff

在 `x86_64` 上复原原始 handoff 设计后，`concurrency_stress` 仍会卡死。

这说明：

- 问题不是某个特定架构偶发实现问题
- 原始 handoff 协议本身就存在收敛缺陷

### 17.2 仅补跨 CPU 调度，不改 handoff 协议

在 `x86_64` 上，只给原始 handoff 增加远端 `resched` IPI 后，并不能稳定通过。

更早暴露出的失败形式是：

- 目标 waiter 被更及时地调度到
- 但原始 handoff 的协议不闭合问题反而更快显现
- 甚至触发“线程试图再次获取自己已拥有的 mutex”的断言

这说明：

> 跨 CPU 调度推进是必要条件，但不是充分条件。

### 17.3 两阶段 handoff + 远端 `resched` IPI

在 `x86_64` 上，将：

- 两阶段 handoff 状态机
- 远端 CPU 的 `resched` IPI

组合后，`concurrency_stress` 已能稳定通过，并且 handoff accept 比例显著高于 fallback。

多轮回归结果表明：

- `accept_pct` 稳定在约 `93%` 到 `96%`
- handoff 已成为主路径
- fallback 仅作为小比例兜底路径存在

因此，当前最可靠的实现结论是：

> 想让 handoff 在多核下真正工作，必须同时满足“协议闭环”和“远端 CPU 可被及时推动调度”这两个条件。

---

## 18. 一句话总结

> 第三版 handoff 状态机的核心是：把“我打算把锁交给 T2”和“T2 已经真正接手锁”拆成两个显式状态，并通过统一原子状态 + epoch + CAS 保证协议收敛；而在 SMP 下，还必须配合远端 CPU 的调度推进能力，handoff 才能真正高比例生效并避免功能性卡死。
