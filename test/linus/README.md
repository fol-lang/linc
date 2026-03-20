# Linux Code-Driven Examples

This directory carries Linux-focused, code-driven integration examples for `bic`.

These examples are intentionally library-first:

- no sidecar config files
- no CLI assumptions
- no generated manifests

The goal is to show what a downstream consumer such as `fol` would construct directly in Rust.

Current examples:

- `socketcan.rs`: analyze the Linux SocketCAN headers, attach explicit Linux/link metadata, and
  request ABI-sensitive layout probes entirely from code
- `epoll.rs`: analyze `sys/epoll.h`, attach Linux/libc metadata, and request a layout probe for
  `struct epoll_event`
- `linux_event_loop.rs`: combine `epoll`, `timerfd`, and `signalfd` headers into one code-driven
  event-loop-style system example

## SocketCAN Runtime Boundary

SocketCAN is intentionally useful here because it is not a standalone library boundary.

What comes from header analysis:

- constants and macros from `linux/can.h` and `linux/can/raw.h`
- record layouts such as `struct can_frame`, `struct canfd_frame`, and `struct sockaddr_can`
- Linux-only native metadata such as the `linux` platform constraint and the explicit `c` link
  requirement attached in code

What comes from runtime behavior:

- actual socket creation happens through the libc/kernel entry point
  `socket(AF_CAN, SOCK_RAW, CAN_RAW)`
- success or expected kernel errors such as unsupported-address-family results are runtime facts,
  not declaration-extraction facts

The example keeps those layers separate on purpose:

- `analyze_socketcan()` exercises the header-analysis path
- `socketcan_runtime_smoke_check()` exercises the syscall/libc boundary

That split is important for downstream consumers such as `fol`.
They should use `bic` to understand the C surface and native requirements, then apply their own
runtime/build policy on top.

## Planned Torture Target

The synthetic torture target is meant to concentrate difficult C interop constructs into one
header-level surface so `bic` limitations are easier to observe and classify.

The first version is intended to include:

- typedef chains and alias-mediated records
- anonymous nested structs and unions
- bitfields and packed records
- flexible array members
- opaque forward declarations
- function-pointer callbacks
- variadic functions
- macro constants and ABI-affecting configuration macros
- one or more intentionally unsupported declarations

The purpose is not realism.
The purpose is to force one scan to answer:

- what extracted cleanly
- what extracted partially with diagnostics
- what was represented as unsupported
- what can be layout-probed with high confidence

## First Torture Findings

Current first-pass findings from [c_interop_torture.h](/home/bresilla/data/code/bresilla/bic/test/linus/c_interop_torture.h):

- the header preprocesses cleanly through `HeaderConfig`
- the public declarations remain visible in `PreprocessingReport.preprocessed_source`
- the declaration surface does not currently extract into `BindingPackage.items`
- the blocking construct is the packed typedef form:
  `typedef struct TORTURE_PACKED torture_packet { ... } torture_packet;`
- the package now records that failure explicitly as one `ParseFailed` diagnostic
- requested ABI probes still run on this path, so layout evidence is retained even when parsing
  fails after preprocessing
- macro capture also survives this path, so the package still retains evidence such as
  `TORTURE_API_LEVEL`

What this means today:

- `bic` can still provide useful compiler- and preprocessor-backed evidence from a parser-hostile
  header
- downstream consumers should distinguish declaration extraction success from probe and macro
  evidence
- the next improvement area is parser coverage for attribute-bearing typedef forms, especially
  packed record declarations that place attributes between `struct` and the tag name
