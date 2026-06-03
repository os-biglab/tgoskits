# StarryOS 大作业总结报告

---

## 1. biglab-A 总结：内核态中断与多核

biglab-A 主要学习了两个核心知识点：

**内核态中断处理**：理解了中断在内核中的完整生命周期——从硬件触发 IRQ，到中断控制器分发，到内核的中断处理函数执行，再到中断返回。重点掌握了中断上下文与进程上下文的区别：中断上下文不能睡眠、不能获取可能睡眠的锁、需要关中断保护共享数据结构。在 StarryOS 中，网络设备的 IRQ 处理函数（如 `handle_ethernet_irq`）只做最少的工作——设置 pending 标志并唤醒等待队列，真正的数据处理推迟到任务上下文中执行（`poll_interfaces()`）。

**多核调度**：学习了 StarryOS 的 per-CPU 运行队列设计。每个 CPU 核心有自己的 `AxRunQueue`，任务在创建时绑定到某个 CPU。跨 CPU 唤醒通过 IPI（Inter-Processor Interrupt）实现。抢占式调度通过 `preempt_pending` 标志和 `resched()` 函数在安全点触发上下文切换。这些机制是后续 jcode 支持中多个修复（抢占式唤醒、双重入队）的基础。

---

## 2. biglab-B 总结

### 实验 1：os-plugin

**功能**：os-plugin 是 ArceOS 的模块化插件系统，允许在不修改内核核心代码的情况下扩展功能。ArceOS 采用微内核风格的模块化设计，文件系统、网络栈、内存管理等都是独立的 crate，通过 feature flag 组合。

**优势**：相比传统宏内核（如 Linux），ArceOS 的模块化设计使得：
- 每个模块可以独立编译和测试
- 可以根据需求裁剪功能（嵌入式场景去掉网络栈）
- 模块间依赖清晰，通过 trait 接口解耦
- 新增功能（如新的文件系统）只需实现对应 trait，不需要修改内核其他部分

### 实验 2：syscall — clone3

**修复内容**：修复 StarryOS 的 `clone3` 参数读取边界问题。

**问题**：`clone3` 系统调用接收一个用户态传入的 `clone_args` 结构体。当前实现会在用户传入的 `size` 大于结构体实际大小时，直接对固定缓冲区做超范围切片，触发内核 panic。恶意用户程序可以通过传入超大 `size` 来使内核崩溃。

**修复**：将读取长度限制为缓冲区实际大小，保持对超长参数的兼容性，避免内核崩溃。

```rust
// 修复：限制读取长度
let len = size.min(core::mem::size_of::<clone_args>());
```

**思考**：这是典型的内核安全问题——用户态传入的参数不可信，必须做边界检查。Linux 内核对所有 `copy_from_user` 调用都有严格的长度校验。

### 实验 3：Linux 小应用支持 — top 与 procfs

**背景**：多个 Linux 标准命令在 StarryOS 上完全无法运行，根本原因是 procfs 缺少关键文件或返回硬编码假数据：
- `top` → `can't open 'stat': No such file or directory`（启动即崩溃）
- `uptime` → 显示 0min
- `cat /proc/cpuinfo` → No such file or directory
- `free` → 显示乱码（与实际 512 MB QEMU 内存完全不符）

**新增 procfs 文件**：

| 文件 | 内容 |
|------|------|
| `/proc/stat` | CPU user/system/idle jiffies（从 `TimeManager` 读取真实数据）；`procs_running` 统计 Running/Ready 态任务；`procs_blocked` 统计 Blocked 态 |
| `/proc/cpuinfo` | 支持 riscv64/aarch64/x86_64/loongarch64 四种架构的 CPU 信息 |
| `/proc/uptime` | 格式 `{secs}.{cs} {idle}.00`，使用单调时钟 |

**修复已有条目**：

| 文件 | 修复 |
|------|------|
| `/proc/meminfo` | 替换硬编码 32 GB 常量，改用 `ax_hal::mem::total_ram_size()` 和 `ax_alloc` 使用量追踪计算真实值 |
| `/proc/interrupts` | 将 `IRQ_CNT` 提升为模块级 static，与 `/proc/stat` 共享 |

**CPU 时间计账改进**：新增 `TimeManager::tick()` IRQ 安全方法和 `tick_cpu_time()` per-CPU timer callback，使被抢占的任务在每个 tick 都记录 CPU 时间，而不必等到下一次 syscall 边界。

### 实验 4：jcode 支持

#### jcode 放入文件系统的完整流程

jcode 是一个 TUI 应用，用 glibc 编译，但 StarryOS 的用户态用 musl libc。放入文件系统需要解决 glibc/musl 兼容问题。

**第一步：准备资产（`prepare_jcode_assets.sh`）**

1. 下载 jcode 发布包（`jcode-linux-x86_64.tar.gz`），解压得到 `jcode.bin`
2. 下载 Alpine minirootfs，解压为 staging 根目录
3. 在 staging 里用 `apk` 安装依赖（`patchelf`、`krb5-libs`）
4. `patchelf` 修改 `jcode.bin`：把 glibc 动态链接器换成 musl，把 `libc.so.6` 换成 `libc.musl-x86_64.so.1`
5. 编译 glibc 符号桩（`libglibc_stub.so`）：手写汇编实现 `mallopt`、`malloc_trim` 等 glibc 特有函数，注入 `jcode.bin` 的 NEEDED
6. 创建启动脚本 `/usr/bin/jcode`

**第二步：注入 rootfs（`prepare_jcode_rootfs.sh`）**

1. 复制基础 Alpine rootfs 镜像
2. 用 `debugfs`（ext4 文件系统调试工具）直接往 ext4 镜像里写文件，不需要挂载

**第三步：运行（`qemu-x86_64.toml`）**

QEMU 启动时将 rootfs 镜像挂载为磁盘，StarryOS 内核启动后挂载 ext4 根文件系统，`/usr/bin/jcode` 即可执行。

#### 修复的 PR（重点）

jcode 的运行暴露了 StarryOS 内核的 17 个 bug，涉及 7 个内核模块。以下按模块详细说明每个修复的原理。

##### 1. ext4 文件系统（rsext4）

**Fix 1a：`alloc_blocks` 跳过描述符/位图不一致的块组**

问题：Alpine rootfs 镜像经非正常关机后，块组 1-2 的 BGD 声称有空闲块，但位图全满。原始代码遇到 `ENOSPC` 直接报错，导致整个文件系统报"磁盘已满"。

修复：对 `ENOSPC` 单独处理，跳过不一致的块组继续尝试后续块组。

**Fix 1b：`write_at` 改用 inode 号写入**

问题：jcode 使用原子替换模式保存文件（`write tmp → rename tmp → target`）。VFS 层的 Inode 对象缓存了 `path` 字段，rename 后路径失效，Drop 时 sync 用旧路径写入 → `ENOENT`。

修复：`write_at` 改用 `write_inode_data(dev, fs, self.ino, ...)` 而非 `write_file(dev, fs, &self.path, ...)`，inode 号不受 rename 影响。

**Fix 1c：JBD2 日志超级块损坏处理**

问题：Alpine rootfs 的 JBD2 超级块被损坏：`s_maxlen=1`（实际 8192 块），`s_start=8192`（超出范围）。导致挂载直接失败。

修复：检测 `s_maxlen < s_first`（明显损坏）→ 改用物理分区实际长度；检测 `s_start >= journal_blocks.len()` → 视为"日志干净"，跳过重放。

##### 2. 内存管理与 COW 缺页处理

**Fix 2：epoll/poll/select 内核态 COW 写故障**

问题：原始 `sys_poll`、`sys_select` 等使用 `get_as_mut_slice()` 获取直接指向用户空间的 `&mut` 引用，然后在内核态写入。如果另一个线程在此期间调用 `fork()`，页面被标记为 COW 只读，内核写入触发 #PF → panic。

修复策略一（主修复）：epoll/poll 改用内核缓冲区 + `vm_write_slice` 写回，避免直接写用户内存。

修复策略二（兜底防御）：扩展 `handle_page_fault`，让内核态缺页时也能处理用户空间地址的 COW——检查 vaddr 是否在用户空间范围内，若是且当前线程未持有 aspace 锁，则走 COW 处理路径。

##### 3. epoll 事件通知机制

**Fix 3a：EPOLLET 竞争窗口**

问题：ET 模式下，`mark_not_in_queue()` 和 `register_waker_only()` 之间存在竞争窗口。如果数据在此窗口内到达，且旧 Waker 已消耗、新 Waker 未注册，通知会丢失。

修复：注册好新 waker 后，再次检查文件状态。如果数据已在缓冲区，直接将 interest 入队并唤醒 epoll_wait。

**Fix 3b：NoEvent 路径死循环**

问题：已连接 TCP socket 的 EPOLLOUT 始终就绪。当事件不是想要的（如 EPOLLIN 事件触发但实际只有 EPOLLOUT），`check_and_register_waker()` 会 poll 文件状态并在非空时立即调用 `waker.wake_by_ref()`，形成 NoEvent → wake → re-queue → NoEvent 的死循环。

修复：NoEvent 路径改用 `register_waker_only()`，只安装 waker 而不主动 poll/唤醒，等待下一次真正的状态转换才触发。

##### 4. 异步任务调度与唤醒

**Fix 4：非阻塞 socket 的 Waker 注册**

问题：`poll_io` 对非阻塞模式在 `WouldBlock` 时直接返回，没有注册 Waker。结合 TCP socket 的 `device_mask` 生命周期缺陷，导致 epoll 永远收不到连接完成通知。

核心概念：TCP socket 的 `device_mask` 默认为 0，只有在 `start_connect()` 内部才被设置。如果 `epoll_ctl(ADD)` 在 `connect()` 之前调用（Tokio 的典型流程），InterestWaker 注册到 `mask=0` 上等于没注册。之后 `connect()` 设置了 mask，但原来注册的 InterestWaker 不会自动迁移。

修复：无论阻塞/非阻塞，都先注册 Waker。`connect()` 在 `device_mask` 设好之后再注册一次，确保 Waker 落到正确的 device 上。

**Fix 5a：AxWaker 改为抢占式唤醒**

问题：原始 `unblock_task(task, false)` 不设置抢占标志，被唤醒的任务必须等当前任务主动让出 CPU 才能运行。键盘响应延迟高达 20ms。

修复：`unblock_task(task, false)` → `unblock_task(task, true)`，立即抢占。

**Fix 5b：防止任务双重加入等待队列**

问题：启用抢占式唤醒后，任务 T 在 `wait_until` 的 `blocked_resched` 中被唤醒（`resched=true`），立刻抢占回来，但还在 `wait_until` 的 loop 里，条件仍不满足（锁还没释放），试图再次 `push_back` → 产生重复条目。

修复：在 `push_back` 前检查 `in_wait_queue` 标志，已在队列中则跳过。

**为什么"条件仍不满足"？** 因为 `notify_one(resched=true)` 在持有锁的代码中调用，唤醒 T 后锁还没释放。抢占式唤醒让 T 立刻运行，但锁还在别人手里，`condition()` 返回 false。

##### 5. 网络协议栈

**Fix 6a：ARP 待发包队列扩容（32 → 128）**

问题：jcode 启动时并发建立 10-20 个 TCP 连接，32 个槽位耗尽，多余 SYN 包被静默丢弃。

**Fix 6b：ARP 缓存 TTL 延长（60s → 300s）**

问题：AI 响应可能超过 60s，ARP 缓存过期后所有 ACK 包重新排队，压爆队列。

**Fix 6c：ARP 回复后全队列扫描**

问题：原始代码只处理队头，如果队列是 `[A, B, A, A]`（A 已解析，B 未解析），只发送第一个 A，后续 A 永远卡在 B 后面（Head-of-Line Blocking）。

修复：ARP 回复到达时扫描整个队列，发送所有已解析的包，未解析的重新排队。

**Fix 6d：Unix socket 非阻塞 accept**

问题：原始 `accept()` 始终阻塞，忽略 `O_NONBLOCK`。Tokio 用 EPOLLET 模式，收到 EPOLLIN 后循环 accept 直到 EAGAIN，但旧代码的 accept 永远不返回 → 整个 Tokio runtime 冻结。

修复：添加 `try_accept()`，`accept()` 改用 `poll_io + try_accept()`，无连接时立即返回 `WouldBlock`。

**Fix 6e：Unix socket 关闭时 EOF 信号顺序**

问题：`StreamTransport` 被 drop 时，`HeapProd` 还没被 drop（Rust 的字段 drop 在函数体之后执行）。对端被唤醒后 `poll()` 看到 `write_is_held()=true` + 空缓冲区 → 报告无事件 → 永远等不到第二次通知。

修复：添加 `my_tx_closed` / `peer_tx_closed` 标志，在 `wake()` 之前设置。对端 `poll()` 读到 `peer_tx_closed=true` 时直接报告 EOF，不依赖 `write_is_held()` 状态。

##### 6. TTY 终端子系统

**Fix 7a：查询真实终端尺寸**

问题：原始 NTty 默认窗口 28x110，与实际终端不匹配 → ratatui 布局错误。

修复：启动时通过 ANSI CPR（Cursor Position Report）查询宿主终端真实尺寸——发送 `\x1b[9999;9999H` 把光标移到极右下角（停在终端边界），再发 `\x1b[6n` 请求光标位置，终端回复 `\x1b[rows;colsR`。

**Fix 7b：TIOCSWINSZ 发送 SIGWINCH**

问题：原始实现只修改 `window_size`，不发 `SIGWINCH`。调整终端窗口后 TUI 不刷新。

修复：`TIOCSWINSZ` 处理中，检查尺寸是否真正改变，若改变则发送 `SIGWINCH` 到前台进程组。TUI 程序收到信号后重新读取尺寸并刷新界面。

**Fix 7c：批量 UART 输出**

问题：每个 `\n` 两次 UART 写（ONLCR 转换 `\n` → `\r\n`），每帧数十次锁获取/释放 → 终端逐行渲染 → 画面闪烁。

修复：先收集到 `Vec`，一次写出。x86 平台 `Console` 改为在 `write_bytes` 期间持有整个 COM1 锁，确保一帧原子发送。

##### 7. ostool 鼠标事件转发

**Fix 8a：添加鼠标事件转发**

问题：jcode 启用 `EnableMouseCapture` 后，宿主机终端产生 SGR 鼠标事件，但 ostool 静默丢弃。

修复：ostool 进入交互模式时启用鼠标捕获，遇到鼠标事件时编码为 SGR 格式（`\x1b[<Cb;Cx;CyM/m`）写入 QEMU stdin，QEMU 把它当作串口输入传给 StarryOS 内核。

**Fix 8b：过滤 Moved 事件**

问题：`MouseEventKind::Moved` 以极高频率触发（鼠标每移动一像素一个事件）→ 数百次 TUI 重绘 → UART 饱和 → 界面冻结。

修复：丢弃 `Moved` 事件，只转发点击/滚动/拖拽。

#### 修复汇总表

| 编号 | 模块 | 问题 | 修复 |
|------|------|------|------|
| 1a | 文件系统 | alloc_blocks 遇不一致立即报 ENOSPC | 跳过不一致块组 |
| 1b | 文件系统 | rename 后 write_at 路径失效 | 改用 inode 号 |
| 1c | 文件系统 | JBD2 日志超级块损坏，挂载失败 | 安全回退 |
| 2 | 内存管理 | poll/select/epoll COW 缺页 panic | 内核缓冲区 + 兜底 COW |
| 3a | epoll | EPOLLET 竞争窗口，通知丢失 | 注册后重新检查数据 |
| 3b | epoll | NoEvent 路径死循环 | 只注册 Waker 不主动唤醒 |
| 4 | 调度器 | device_mask=0 时 epoll_ctl 注册无效 | 无条件注册 Waker |
| 5a | 调度器 | AxWaker 不抢占，响应慢 20ms | resched=true 立即抢占 |
| 5b | 调度器 | 抢占导致双重入队 panic | 检查 in_wait_queue |
| 6a | 网络 | ARP 队列 32 包溢出 | 扩容到 128 |
| 6b | 网络 | ARP 缓存 60s 过期 | TTL 延长到 300s |
| 6c | 网络 | ARP 回复只处理队头 | 全队列扫描 |
| 6d | 网络 | Unix socket accept 忽略 O_NONBLOCK | 实现 try_accept |
| 6e | 网络 | Unix socket 关闭时 EOF 信号延迟 | Drop 时先设 closed 标志再 wake |
| 7a | TTY | TIOCGWINSZ 返回硬编码尺寸 | ANSI CPR 查询真实尺寸 |
| 7b | TTY | TIOCSWINSZ 不发 SIGWINCH | 发送 SIGWINCH |
| 7c | TTY | 每个换行符多次 UART 写 | 批量输出 |
| 8a | ostool | 鼠标事件被丢弃 | SGR 编码转发 |
| 8b | ostool | Moved 事件风暴冻结 TUI | 过滤 Moved |

#### 关键收获

通过 jcode 支持的调试过程，我深入理解了 OS 内核的以下核心概念：

1. **VFS 与文件系统的分层**：VFS 的 DirEntry 树（内存缓存）、Inode 对象、ext4 目录块（磁盘）三层结构。Inode 不应该缓存 path，因为 rename 只更新 DirEntry 树和磁盘，不更新 Inode。

2. **异步执行模型**：Rust Future 是 poll-based 状态机，`block_on` 是驱动器。`poll_io` 是通用 I/O 等待函数，通过 `PollSet`（Waker 集合）与 epoll/poll/select 配合。

3. **Waker 注册的生命周期**：`device_mask` 决定 Waker 注册到哪个网卡设备，mask 在 `connect()` 时才设置，如果 `epoll_ctl(ADD)` 在 `connect()` 之前，InterestWaker 注册到空 mask 上等于没注册。

4. **抢占式调度的副作用**：抢占式唤醒提高响应速度，但引入了新的竞争条件——任务可能在条件还不满足时就被唤醒，需要在入队时检查状态。

5. **Unix socket 的 EOF 检测**：Rust 的字段 drop 顺序（函数体先执行，字段后 drop）导致 `HeapProd` 在 `wake()` 时还活着，需要用原子标志绕过这个限制。

#### jcode 启动全链路：修复在 QEMU 运行过程中的位置

下图展示 jcode 从启动到正常运行的完整路径，标注每个模块的修复在实际运行中何时触发：

```
用户在 QEMU 终端输入 `jcode`
    │
    ▼
┌─[文件系统] ext4 挂载 ─────────────────────────────┐
│  JBD2 日志超级块损坏被安全忽略        (Fix 1c)     │
│  alloc_blocks 跳过不一致块组          (Fix 1a)     │
│  读取 /usr/bin/jcode 启动脚本                      │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[TTY] NTty 初始化 ────────────────────────────────┐
│  通过 ANSI CPR 查询到宿主机真实终端尺寸  (Fix 7a)  │
│  默认值改为 24×80                      (Fix 7a)    │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[jcode TUI 进程] 启动 ────────────────────────────┐
│  EnableMouseCapture → ostool 请求鼠标   (Fix 8a)   │
│  ioctl(TIOCGWINSZ) → 获取真实尺寸       (Fix 7a)   │
│  ratatui 布局正确                                   │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[jcode serve 进程] 启动 ──────────────────────────┐
│  Unix socket 监听 (O_NONBLOCK)          (Fix 6d)   │
│  向 TUI 进程写"就绪"信号                            │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[TUI ↔ serve] Unix socket IPC ────────────────────┐
│  EPOLLET 竞争窗口已修复                 (Fix 3a)    │
│  AxWaker 立即抢占，键盘即时响应         (Fix 5a)    │
│  双重等待队列 panic 已修复              (Fix 5b)    │
│  关闭时 EOF 信号顺序正确                (Fix 6e)    │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[serve 发出 HTTPS 请求] ──────────────────────────┐
│  非阻塞 connect Waker 正确注册          (Fix 4)     │
│  ARP 队列足够大                         (Fix 6a)    │
│  TCP 握手完成后 epoll 正确通知          (Fix 4)     │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[AI 响应流回来] ──────────────────────────────────┐
│  ARP 缓存 300s 不过期                   (Fix 6b)    │
│  ARP 队列扫描发送                       (Fix 6c)    │
│  epoll EPOLLET 连续消息正常通知         (Fix 3b)    │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[会话保存] ───────────────────────────────────────┐
│  rename 后 write_at 用 inode 号         (Fix 1b)    │
│  TUI 正常更新（不挂在 loading）                     │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[用户鼠标操作] ───────────────────────────────────┐
│  ostool 转发点击/滚动                   (Fix 8a)    │
│  Moved 事件被过滤                       (Fix 8b)    │
│  TIOCSWINSZ 发送 SIGWINCH               (Fix 7b)    │
└────────────────────────────────────────────────────┘
    │
    ▼
┌─[TUI 输出到终端] ────────────────────────────────┐
│  批量 UART 写，整帧原子发送             (Fix 7c)    │
│  COW 缺页安全处理                       (Fix 2)     │
└────────────────────────────────────────────────────┘
```

---

## 3. 总结：AI 能力快速发展下 OS 课教学/实验设计思考

### 当前 OS 教学的挑战

AI 代码生成工具（如 Copilot、Claude）已经能够自动完成大量"标准"OS 编程任务——实现 syscall、写设备驱动、修 bug。传统的"实现一个简单的 xxx"型实验正在失去价值，因为 AI 可以直接生成正确代码。

### 建议的实验设计方向

**1. 从"实现"转向"理解与调试"**

与其让学生从零实现一个 syscall，不如给学生一个有 bug 的内核，让他们通过调试定位问题。jcode 支持的 17 个修复就是很好的案例——每个 bug 都需要理解内核模块的交互、锁的生命周期、异步执行模型才能定位。

**2. 跨模块系统性问题**

传统的 OS 实验往往是单模块的（写文件系统、写调度器）。真正有价值的练习是跨模块问题——比如 Fix 4（device_mask 生命周期）涉及网络栈、epoll、异步调度三个模块的交互。这类问题 AI 也很难直接解决，因为需要理解整个系统的状态机。

**3. 真实应用驱动**

用真实应用（如 jcode）作为测试用例，而不是人造的单元测试。真实应用会触发内核的边界条件、竞争条件、资源耗尽等场景，这些是单元测试覆盖不到的。

**4. 性能调优**

从"功能正确"到"性能优化"。Fix 5a（抢占式唤醒降低延迟）、Fix 7c（批量 UART 输出消除闪烁）都是性能优化案例。学生需要学会 profiling、理解硬件特性（UART 锁开销）、权衡设计决策。

**5. 安全性意识**

Fix 2（COW 缺页 panic）、clone3 边界检查等案例展示了内核安全的重要性。AI 生成的代码往往缺少安全检查，学生需要学会审查 AI 生成的代码。

---

## 附录：代码导航表

| 功能 | 文件 | 关键函数 |
|------|------|---------|
| 块分配 | `components/rsext4/src/ext4/alloc.rs` | `alloc_blocks` |
| 文件写入 | `components/rsext4/src/file/io.rs` | `write_inode_data` |
| VFS inode 适配 | `os/arceos/modules/axfs-ng/src/fs/ext4/rsext4/inode.rs` | `write_at` |
| JBD2 日志重放 | `components/rsext4/src/jbd2/jbd2.rs` | `replay_with_mapping` |
| 缺页处理 | `os/StarryOS/kernel/src/mm/access.rs` | `handle_page_fault` |
| epoll 核心 | `os/StarryOS/kernel/src/file/epoll.rs` | `poll_events_with` |
| poll_io | `os/arceos/modules/axtask/src/future/poll.rs` | `poll_io` |
| AxWaker | `os/arceos/modules/axtask/src/future/mod.rs` | `wake_by_ref` |
| 任务调度 | `os/arceos/modules/axtask/src/run_queue.rs` | `unblock_task` |
| 等待队列 | `os/arceos/modules/axtask/src/wait_queue.rs` | `wait_until` |
| ARP 设备 | `os/arceos/modules/axnet-ng/src/device/ethernet.rs` | `send` / `process_arp` |
| Unix 流传输 | `os/arceos/modules/axnet-ng/src/unix/stream.rs` | `try_accept` / `Drop` |
| N_TTY | `os/StarryOS/kernel/src/pseudofs/dev/tty/ntty.rs` | `query_console_size` |
| TTY ioctl | `os/StarryOS/kernel/src/pseudofs/dev/tty/mod.rs` | `ioctl` |
| 行规程输出 | `os/StarryOS/kernel/src/pseudofs/dev/tty/terminal/ldisc.rs` | `write_output_bytes` |
| ostool 鼠标 | `ostool/src/sterm/mod.rs` | `encode_mouse_event` |
| procfs | `os/StarryOS/kernel/src/pseudofs/proc.rs` | 各 procfs 文件 |
| clone3 | `os/StarryOS/kernel/src/syscall/task/clone3.rs` | `sys_clone3` |
