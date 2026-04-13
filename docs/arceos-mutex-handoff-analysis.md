# ArceOS RawMutex 多核并发卡死问题分析与后续改造方向

## 1. 背景

在 ArceOS 的多核场景下，`axsync::RawMutex` 使用了基于 waiter 定向交接的 handoff 语义：

- `unlock()` 时，不是先把锁释放为无主状态再唤醒等待者
- 而是直接把 `owner_id` 改成被选中的 waiter 的 task id
- waiter 被唤醒后，通过观察 `owner_id == current_id` 判断自己已经接手锁

这套设计的初衷是：

- 减少重新竞争
- 改善公平性
- 避免 waiter 被唤醒后再和 newcomer 竞争锁

但在多核高并发场景下，我们观察到了概率性卡死现象。

---

## 2. 问题现象

在 `SMP=8`、RR 调度下，系统会出现如下现象：

- 某一轮 barrier 已经 release
- 一部分 worker 已经进入下一轮等待
- 大量 worker 停留在获取共享 mutex 的阶段
- `progress` 长时间不再增长
- watchdog 最终因为“无前进”触发 panic

典型表现为：

- `released` 持续推进到某一轮后停止
- `arrived` 略高于 `released * worker_num`
- `progress` 低于 `released * worker_num`
- `timeouts` 持续增长

这说明系统不是彻底死机，而是：

- 一部分线程仍在 timeout / wake / retry
- 但整体推进链已经断掉

---

## 3. 压测测例

为了更高概率触发问题，我们增加了专门的测例：

- 路径：`test-suit/arceos/rust/task/concurrency_stress`

这个测例做的事情：

1. 启动大量 worker
2. 每轮通过 `WaitQueue` 做 barrier 同步
3. release 后所有 worker 竞争同一个共享 `Mutex<u64>`
4. 在部分实验中，故意在持锁阶段 `yield_now()`
5. 通过 watchdog 观察是否出现“长时间无前进”

这个测例的目标不是模拟正常业务，而是主动放大同步原语和调度交互中的薄弱时序。

---

## 4. 定位过程

### 4.1 初步现象判断

一开始只能判断：

- 问题发生在 `release` 之后、`progress` 增长之前
- 怀疑点包括：
  - `WaitQueue`
  - `run_queue` 唤醒 / 调度
  - `RawMutex` handoff 语义

### 4.2 通过阶段追踪缩小范围

在测例中为每个 worker 增加阶段状态：

- `wait_start`
- `before_release`
- `after_release`
- `before_lock`
- `in_mutex`
- `after_mutex`
- `done`

失败现场中，多次出现：

- `before_release > 0`
- `before_lock > 0`
- `in_mutex = 0`
- `after_mutex = 0`

这说明：

- 某一轮已经 release
- 大量线程已经通过 `wait_for_release()`
- 但没有线程真正进入临界区继续推进

因此问题被缩小到：

> `release` 之后到 `Mutex::lock()` 成功接管之间

### 4.3 排除“waiter 没被唤醒 / 没被调度”的可能

为了进一步判断 handoff waiter `T2` 到底卡在哪一步，我们加入了内核日志：

- `mutex_handoff_task_scheduled`
- `mutex_wait_return`
- `mutex_handoff_seen_owner`
- `mutex_lock_returning_after_handoff`

这些日志证明：

1. handoff 选中的 waiter 会被成功 unblock
2. waiter 会被 scheduler 选中运行
3. waiter 从 `wait_until()` 返回时，能观察到 `owner_id == 自己`
4. waiter 甚至可以从 `lock()` 返回

因此可以排除：

- 不是 waiter 没被唤醒
- 不是 waiter 没被调度
- 不是 waiter 看不到自己是 owner

### 4.4 对照实验

我们做了几个关键对照：

#### 对照 A：关闭迁移

- `migration=false`
- 问题仍然复现

结论：

- 迁移会放大问题
- 但迁移不是根因

#### 对照 B：关闭持锁时 `yield_now()`

- `migration=false`
- `yield_in_cs=false`
- 测例可以跑通

结论：

- 持锁时 `yield_now()` 是非常强的触发放大器
- 但它仍然只是放大器，不是 handoff 设计本身的替代解释

#### 对照 C：取消 handoff，改成释放后重新竞争

将 `unlock()` 改成：

```rust
self.owner_id.store(0, Ordering::Release);
self.wq.notify_one(true);
```

结果：

- 同一测例能够顺利完成

结论：

- 问题与 handoff 协议强相关
- 测例本身的控制流不是根因

---

## 5. 结论：根本原因

当前 `RawMutex` 的 handoff 设计存在“语义不闭合”的问题。

更准确地说：

- 当前实现把 handoff 定义为：
  1. `unlock()` 选择一个 waiter
  2. 直接写 `owner_id = waiter_id`
  3. waiter 被唤醒后看到 `owner_id == current_id`
  4. waiter 从 `lock()` 返回

但实验说明：

> 即使以上步骤都发生了，系统仍然可能无法稳定前进。

因此，当前 handoff 只完成了**局部所有权转移**，没有完成**全局推进状态收敛**。

也就是说，系统实际上缺少一个“handoff 完成确认”的阶段：

- `owner_id` 虽然已经改成 waiter
- waiter 也可能已经从 `lock()` 返回
- 但从系统整体看，锁并没有进入一个稳定、可持续推进的新状态

这就是为什么系统会出现：

- 锁在逻辑上已经有 owner
- 其他 waiter 因此不再重新竞争
- 但整体却不再前进

我们将这种现象称为：

> handoff 协议未形成闭环

---

## 6. 为什么多核更容易出现问题

多核下，这个 handoff 窗口更容易被放大，原因包括：

- waiter 唤醒和真正运行分离
- 多个 CPU 上的 run queue / wait queue / timeout / wake 交错更多
- 持锁线程主动 `yield_now()` 会让锁前堆积更多 waiter
- handoff 后其他线程不会重新竞争，系统推进依赖被选中的那个 waiter

因此，多核不是根因本身，但它会显著提高问题暴露概率。

---

## 7. 改造方向：从“单阶段 handoff”改为“带确认的两阶段 handoff”

我们的目标不是放弃 handoff，而是保留 handoff 的公平性和性能方向，同时修复其在多核并发场景下的脆弱性。

建议方向：

### 单阶段 handoff（现状）

现状相当于：

1. 选择 waiter `T2`
2. 直接把 `owner_id` 改成 `T2`
3. 认为 handoff 已完成

问题是：

- `owner_id` 改变 != 系统状态已稳定收敛

### 两阶段 handoff（建议）

建议改造为：

1. `unlock()` 只发起 handoff
   - 记录 handoff target
   - 唤醒目标 waiter
2. waiter 真正运行并确认接管后
   - 再提交 handoff 完成
   - 更新最终 owner 状态

核心思想是把：

- “逻辑上指定下一任 owner”

和

- “新 owner 真的完成接管”

分成两个阶段，而不是混成一个原子语义。

---

## 8. 建议的实现原则

后续改造时应尽量满足以下原则：

1. 不要让 `owner_id` 单独承担全部 handoff 语义
2. 必须区分：
   - handoff 已发起
   - handoff 已确认完成
3. waiter 被选中后，需要有明确的接管确认点
4. 其他线程在 handoff 未确认完成前，不应永久失去恢复竞争的能力
5. 保持状态机简单，避免再引入新的 ABA / 双重接管 / 丢失唤醒竞态

---

## 9. 当前推进建议

建议后续按以下顺序推进：

1. 基于当前结论，设计一个最小的两阶段 handoff 状态机
2. 在 `RawMutex` 中实现第一版两阶段 handoff
3. 用 `concurrency_stress` 测例回归
4. 确认问题消失后，再评估性能和公平性

不建议直接在现有单阶段 handoff 上继续打补丁，因为：

- 当前问题不是单一缺少某条日志或某个时序 if 判断
- 而是协议本身缺少“完成确认”这个阶段

---

## 10. 当前结论一句话总结

> ArceOS 当前 `RawMutex` 的 handoff 机制，在多核高并发场景下只完成了 owner 的局部交接，没有保证系统状态全局收敛；这会导致逻辑 owner 已存在、其他 waiter 又不再竞争，但系统整体不再前进，最终形成概率性并发卡死。

