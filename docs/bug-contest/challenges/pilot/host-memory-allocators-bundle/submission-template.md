# `host-memory-allocators-bundle` 提交说明

本题是 `内存分配审计包` 的试运行 challenge 包，提交时请围绕以下上下文组织说明。

## 1. 补丁摘要

- 修改路径建议限定在：
- `components/axallocator`
- `components/bitmap-allocator`
- `components/range-alloc-arceos`
- 涉及组件：`axallocator`, `bitmap-allocator`, `range-alloc-arceos`
- 你修复了什么问题：

## 2. 根因分析

请说明：

- 问题如何触发。
- 根因属于哪一类：`边界条件`, `双重回收`, `对齐与碎片控制`, `空闲区间合并错误`。
- 为什么这不是单纯的样例失败，而是一个真实缺陷。

## 3. 修复方案

请说明：

- 你改了哪些逻辑。
- 为什么这是最小修复。
- 是否考虑过其他修法，以及为什么没有采用。

## 4. 回归验证

请至少给出你实际运行过的命令与结果摘要：

```bash
cargo test -p axallocator
cargo test -p bitmap-allocator
cargo test -p range-alloc-arceos
```

## 5. 影响分析

请说明这个问题会影响：

- 哪个组件或模块
- 哪类用户或运行场景
- 是否可能波及 `Host` 或其他系统链路
