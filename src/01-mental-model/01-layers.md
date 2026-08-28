# 일곱 계층 mental model

메모리 문제를 진단할 때는 먼저 **어느 계층의 상태를 보고 있는지** 정해야 한다. 같은 시점에도 Rust 객체는 이미 `Drop`되었고, allocator는 해당 block을 재사용 가능 상태로 보관하며, OS의 RSS는 그대로일 수 있다. 세 문장은 동시에 참일 수 있다.

<iframe class="memory-diagram memory-diagram--wide" src="../assets/diagrams/memory-layers.html" title="Rust memory management의 일곱 계층" loading="lazy"></iframe>

[다이어그램을 새 창 크기로 보기](../assets/diagrams/memory-layers.html)

| 계층 | 핵심 질문 | 대표 개념 |
|---|---|---|
| Language | 누가 값을 소유하고 누가 접근할 수 있는가? | ownership, borrowing, lifetime, move |
| Object lifetime | 객체의 destructor는 언제 실행되는가? | scope, `Drop`, RAII |
| Collections & heap | 어떤 type이 얼마의 capacity를 보유하는가? | `Box`, `Vec`, `String`, `HashMap`, `Arc` |
| Allocator | 요청과 반납을 어떤 block으로 관리하는가? | allocate, deallocate, arena, fragmentation, retention |
| OS VM/RSS | 어떤 virtual page가 resident한가? | mapping, page fault, RSS, anonymous/file-backed memory |
| Resource control | 어떤 경계에서 reclaim, throttle, kill하는가? | cgroup `memory.high`, `memory.max`, OOM |
| Server/DB governance | 어떤 작업에 메모리 사용 권한을 줄 것인가? | budget, reservation, admission, spill, eviction |

## 서로 다른 세 가지 lifetime

“메모리가 살아 있다”는 표현은 모호하다. 다음을 구분한다.

1. **reference lifetime:** reference가 유효하게 사용될 수 있는 범위
2. **object lifetime:** 값이 생성된 뒤 destructor가 실행되기 전까지의 논리적 생존 기간
3. **allocation lifetime:** allocator가 block을 할당한 뒤 deallocation 요청을 받기 전까지

여기에 allocator가 반납된 block을 보관하는 기간과 OS page가 resident한 기간이 추가된다. Rust의 lifetime annotation은 이 모든 시간을 표현하지 않는다. 주로 reference 사이의 유효성 관계를 정적 분석에 전달한다.

## 진단 질문을 계층에 배치하기

```text
"이 reference는 유효한가?"           → Language
"왜 destructor가 호출되지 않았나?"    → Object lifetime
"왜 Vec가 비어도 capacity가 남나?"    → Collections
"왜 free 뒤에도 process가 잡고 있나?" → Allocator
"왜 RSS가 내려가지 않나?"             → OS VM/RSS
"왜 container만 OOM kill됐나?"        → Resource control
"왜 query를 미리 거절하지 않았나?"     → Server/DB governance
```

한 질문의 답이 다음 계층의 결과를 자동으로 보장하지 않는다. `Drop` 실행은 allocator에 deallocation을 요청할 수 있지만, RSS 감소나 cgroup 여유 증가를 직접 보장하지 않는다.

## 보장 경계

### 이 장이 보장하는 설명

- ownership, deallocation, RSS, OOM을 서로 다른 계층의 사건으로 분리하는 모델
- 진단을 시작할 계층을 선택하는 질문 목록

### 이 장이 보장하지 않는 것

- 모든 type이 반드시 heap allocation을 수행한다는 주장
- deallocation 직후 allocator가 OS에 page를 반환한다는 주장
- RSS가 application이 소유한 live object의 정확한 합이라는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — What Is Ownership?](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
- **언어 규범:** [Rust Reference — Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **구현 확인:** [`std::alloc`](https://doc.rust-lang.org/std/alloc/index.html)
- **OS 공식:** [Linux proc filesystem](https://docs.kernel.org/filesystems/proc.html)
