# Research, Feasibility, and Related Work

**Research snapshot date:** 12 October 2026. Links should be checked again before implementation because repositories and Android behavior change.

## 1. Key conclusion

The concept is feasible in a **workload-specific** form: a host can send supported jobs to a worker process on another device. The universal form—automatically making every unmodified laptop application consume an attached Android phone's physical RAM, CPU, and GPU as though they were local hardware—is not a realistic MVP and is not established by the related tools below.

The practical development question is: which useful workloads have a clean boundary that lets the host move compute and required inputs to the phone, without moving so much data that transfer overhead erases the benefit?

## 2. Relevant projects and technologies

### Android Debug Bridge (ADB)

Official Android documentation describes port forwarding between a host port and a device port. This provides a reasonable first experiment for communicating with a worker over USB while keeping a custom network service out of the wider network. It is not a promise of raw USB bandwidth or a universal USB networking layer.

Reference: https://developer.android.com/tools/adb

### Termux

Termux provides an Android terminal emulator and Linux-like package environment, including development tooling. It can accelerate the first prototype because it avoids immediately building a full Android application. Android lifecycle, permissions, background execution, and hardware API limits still apply.

Reference: https://termux.dev/en/

### distcc

distcc distributes compatible C/C++/Objective-C compilation jobs across machines. It is a useful example of the core pattern: integrate at a supported task boundary (individual compilation jobs) rather than trying to make an arbitrary running program share CPU transparently. Cross-architecture toolchains, target triples, headers, compiler versions, and Android runtime differences would need specific investigation before treating an Android phone as a build worker.

Reference: https://github.com/distcc/distcc

### llama.cpp RPC backend

llama.cpp documents an RPC backend that exposes remote `ggml` devices and supports distributed inference. The project's own documentation labels this path proof-of-concept, fragile, and insecure for open networks. It demonstrates workload-specific remote compute and model distribution, not arbitrary acceleration of all applications.

Reference: https://github.com/ggml-org/llama.cpp/tree/master/tools/rpc

### Hermit remote memory

Hermit is research software for remote memory/swap using RDMA-capable network hardware and a specialized setup. It illustrates that remote-memory systems can be engineered, but its requirements are very different from a standard Android phone connected over USB-C. It does not provide a ready-made phone-as-laptop-RAM solution.

Reference: https://github.com/uclasystem/hermit

### Linux zram

zram creates compressed RAM-backed block devices and is a local Linux memory-management mechanism. It can be relevant when investigating memory pressure, but it does not borrow phone memory. Compare local zram/swap behavior as a baseline before claiming PocketBridge makes memory pressure better.

Reference: https://www.kernel.org/doc/html/latest/admin-guide/blockdev/zram.html

## 3. What these examples imply

- Distributed compute works best when work can be divided into bounded, relatively independent tasks.
- A worker must support the target runtime and architecture.
- Moving intermediate data over a cable still has a cost.
- Remote memory is not equivalent to local DRAM and may harm latency-sensitive workloads.
- Specialized graphics and AI APIs need their own integration; an Android GPU does not automatically become a Linux GPU device.
- Security and fault handling are part of the core system, not an afterthought.

## 4. Why gaming is the hardest motivating case

A running game tightly couples simulation, memory allocations, rendering, synchronization, GPU drivers, operating-system services, and timing. Arbitrarily moving one thread or memory pages to a phone does not normally help because:

- the process may depend on x86-64 instructions and local libraries;
- the phone uses an ARM64 environment and different APIs;
- main-memory access is latency-sensitive and expects local memory semantics;
- the game's graphics API expects a supported graphics device/driver in the host environment;
- CPU and GPU frame work has synchronization deadlines, so remote latency can cause stutter;
- the actual bottleneck could be GPU throughput, RAM capacity, storage, shader compilation, or thermal throttling rather than offloadable CPU work.

Therefore the project should first diagnose a workload and expose an adapter for a suitable task. A future game-specific approach would require game/runtime integration or a fundamentally different rendering/streaming architecture—not a transparent general-purpose RAM/CPU switch.

## 5. USB-C needs careful wording

USB-C describes connector and cable capabilities, not one fixed data rate or protocol. A cable can be charge-focused or support different data modes and rates. ADB over USB creates a convenient debugging channel, but its actual performance should be measured end-to-end. The project's transport abstraction must permit comparative tests.

## 6. Novelty and positioning

Do not claim that “distributed computing across phones” or “remote memory” is new. Prior work exists in distributed builds, workload-specific RPC, and research remote-memory systems. A plausible project contribution could instead be:

- a practical USB-first host/Android worker experience;
- a safe, explainable scheduler that measures before offloading;
- capability discovery and runtime compatibility across x86-64 Linux and ARM64 Android;
- adapters for user-selected workloads;
- a careful benchmark suite showing when phone-assisted execution does and does not help;
- later, a principled approach to integrating supported applications with low setup burden.

Any public uniqueness or novelty claim should follow a broader literature and product survey, not just this starter list.

## 7. Open research questions

1. What is the real throughput/latency of ADB-forwarded traffic on the target phone and cable?
2. Can a Termux process stay alive reliably enough for long-running jobs under Android power management?
3. What data granularity is needed before phone execution beats local execution?
4. How should the worker report thermal headroom without overusing privileged APIs?
5. Can selected applications expose portable task boundaries without major rewrites?
6. Is remote memory useful over the tested transport for any credible workload, or is it better to use local compression/swap and offload only jobs?
7. Which Android GPU/NPU APIs are accessible without rooting, and for what task types?
