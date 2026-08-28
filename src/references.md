# 참고 문헌

최종 확인일: **2026-08-28**. URL은 가능한 한 project의 공식 문서 또는 source repository를 가리킨다.

## C++ 비교 규범

- [C++ working draft — Dynamic storage allocation](https://eel.is/c++draft/basic.stc.dynamic.allocation) — throwing allocation function의 `std::bad_alloc` failure 계약
- [C++ working draft — Storage allocation errors](https://eel.is/c++draft/new.handler) — `new_handler`가 storage 제공, `bad_alloc` throw, termination 중 하나를 수행하는 계약

## Rust 공식 학습서

- [The Rust Programming Language](https://doc.rust-lang.org/book/) — ownership, borrowing, lifetime, smart pointer의 공식 학습 출처
- [Understanding Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- [한국 Rust 사용자 그룹의 Rust Book 번역](https://doc.rust-kr.org/) — 한국어 용어와 문체 참고. 최신 영문 원문과 version 차이가 있을 수 있음

## 언어 규범 수준

- [The Rust Reference](https://doc.rust-lang.org/reference/) — Rust 언어의 primary reference. 문서 자체의 completeness 고지를 함께 읽어야 함
- [Destructors](https://doc.rust-lang.org/reference/destructors.html)
- [Behavior considered undefined](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)

## Standard library API와 구현 확인

- [`std::alloc`](https://doc.rust-lang.org/std/alloc/index.html)
- [`GlobalAlloc`](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html)
- [`handle_alloc_error`](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)
- [`Vec<T>` guarantees](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees)
- [`TryReserveError`](https://doc.rust-lang.org/std/collections/struct.TryReserveError.html)
- [`rust-lang/rust`](https://github.com/rust-lang/rust) — compiler와 standard library source
- [`alloc::raw_vec`](https://github.com/rust-lang/rust/blob/main/library/alloc/src/raw_vec/mod.rs) — `Vec` buffer의 현재 내부 구현 확인. Public API guarantee와 구분

## 설계 배경

- [RFC 2116 — Alloc Me Maybe](https://github.com/rust-lang/rfcs/blob/master/text/2116-alloc-me-maybe.md) — fallible collection allocation의 목표와 trade-off
- [RFC repository](https://github.com/rust-lang/rfcs) — 승인 RFC의 기록. 현재 구현 상태는 tracking issue/API 문서와 재대조
- [Cargo Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) — 현재 stable profile option 확인

## 고급/unsafe semantics

- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) — advanced/unsafe Rust를 위한 공식 고급 문서. Draft 성격과 오류 가능성 고지를 존중
- [Leaking](https://doc.rust-lang.org/nomicon/leaking.html)
- [Unsafe Code Guidelines Reference](https://rust-lang.github.io/unsafe-code-guidelines/) — aliasing, provenance 등 unsafe semantics의 논의와 glossary. 미확정 내용을 language guarantee로 승격하지 않음
- [Unsafe Code Guidelines repository](https://github.com/rust-lang/unsafe-code-guidelines)

## OS 공식 문서

- [Linux kernel — Memory management concepts](https://docs.kernel.org/admin-guide/mm/concepts.html) — virtual memory, anonymous memory, reclaim의 기본 개념
- [Linux kernel — Overcommit Accounting](https://www.kernel.org/doc/html/latest/mm/overcommit-accounting.html)
- [Linux kernel — Control Group v2](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
- [Linux kernel — Pressure Stall Information](https://www.kernel.org/doc/html/latest/accounting/psi.html)
- [Linux kernel — `/proc` filesystem](https://docs.kernel.org/filesystems/proc.html)
- [Linux man-pages — `proc_pid_status(5)`](https://man7.org/linux/man-pages/man5/proc_pid_status.5.html)
- [Linux man-pages — `proc_pid_smaps(5)`](https://man7.org/linux/man-pages/man5/proc_pid_smaps.5.html)

## 보조 학습 자료

- [The Rust Performance Book](https://nnethercote.github.io/perf-book/) — allocation rate, memory usage, profiling, performance 실무 지침. Normative language specification이 아님
- [Heap Allocations](https://nnethercote.github.io/perf-book/heap-allocations.html)
- [High Assurance Rust](https://highassurance.rs/) / [source](https://github.com/tnballo/high-assurance-rust) — C/C++ 및 computer architecture 배경의 systems programming 학습 자료. Rust 공식 project가 아니며 일부 범위는 WIP일 수 있음
- [mdBook Documentation](https://rust-lang.github.io/mdBook/) — 교재 build와 Rust code example test 도구

## 이 자료들의 빈 공간

각 자료는 자신의 역할에서는 강하지만 다음 전체 chain을 하나의 규범적 문서가 모두 보장하지는 않는다.

```text
ownership
  → Drop
  → collection capacity
  → allocator retention
  → Linux RSS/overcommit
  → cgroup OOM
  → server/DB admission and spill
```

이 책의 역할은 여러 출처를 하나의 mental model로 연결하는 것이다. 연결 과정에서 제시하는 governance interface와 운영 threshold는 공식 Rust 보장이 아니라 명시적인 설계 권고다.
