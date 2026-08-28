# Rust가 막는 문제와 막지 않는 문제

이 표는 이 책의 핵심이다. “Rust는 memory-safe하다”를 “Rust 프로그램은 메모리 때문에 실패하지 않는다”로 확대하면 안 된다.

| 문제 | sound한 safe Rust가 막는가? | 실제 경계 |
|---|:---:|---|
| use-after-free | **예** | 유효하지 않은 reference를 safe code에서 계속 사용할 수 없다. `unsafe`, FFI, soundness bug는 별도다. |
| double free | **예** | ownership과 `Drop` 규칙을 지키는 safe abstraction에서는 한 resource의 중복 해제를 만들 수 없다. |
| dangling reference | **예** | borrow checker가 reference의 유효성 관계를 검사한다. |
| 충돌하는 alias를 통한 mutation | **예** | `&mut T`의 exclusive access 규칙이 핵심이다. interior mutability는 안전한 runtime 규칙 또는 `unsafe` invariant가 필요하다. |
| data race | **예** | safe Rust의 type/trait 규칙은 data race를 막는다. race condition과 deadlock까지 막는 것은 아니다. |
| uninitialized value의 안전하지 않은 읽기 | **예** | safe API만 사용한다는 전제다. `MaybeUninit`와 raw memory는 `unsafe` invariant가 필요하다. |
| memory leak | **아니요** | leak은 memory safety 위반이 아니다. `mem::forget`, 강한 reference cycle, 끝없이 자라는 state로 발생할 수 있다. |
| `Rc`/`Arc` strong cycle | **아니요** | strong count가 0이 되지 않으므로 `Drop`되지 않는다. ownership model이 cycle을 자동 수집하지 않는다. |
| unbounded cache/queue | **아니요** | 논리적으로 유효한 live object가 계속 늘어나는 것은 application policy 문제다. |
| excessive allocation/clone | **아니요** | 정확하지만 비싼 코드는 허용된다. profiling과 API 설계가 필요하다. |
| internal/external fragmentation | **아니요** | allocator의 size class, arena, workload lifetime 분포 문제다. |
| allocator retention | **아니요** | deallocated block을 allocator가 재사용 목적으로 보관할 수 있다. |
| 높은 RSS | **아니요** | RSS에는 heap 이외의 resident page도 포함되며 allocator/OS 정책의 영향을 받는다. |
| recoverable global OOM exception | **아니요** | 일반적인 `std` 기본 allocation failure는 process abort로 이어진다. `try_reserve`처럼 명시적인 fallible 경로는 별도다. |
| process OOM/abort | **아니요** | 일반적인 infallible allocation API는 allocation failure를 정상 `Result`로 돌려주지 않을 수 있다. |
| Linux OOM killer | **아니요** | overcommit 후 page fault 시점의 물리 메모리 부족은 언어 규칙 밖의 문제다. |
| cgroup OOM kill | **아니요** | `memory.max`와 reclaim 조건은 kernel resource control의 영역이다. |
| memory admission | **아니요** | 작업별 budget, reservation, queueing, rejection은 server/DB가 설계해야 한다. |

## 핵심 해석

Rust가 강하게 제공하는 것은 **invalid access를 어렵게 만드는 정적 규칙과 안전한 표준 abstraction**이다. Rust가 자동으로 제공하지 않는 것은 **사용량의 상한과 운영 정책**이다.

```text
memory safety
  "이 access가 유효한가?"
        ≠
memory boundedness
  "이 process가 limit 안에 머무는가?"
        ≠
availability
  "압박 상황에도 service가 계속 응답하는가?"
```

## 세 가지 실제 시나리오

### 시나리오 A — 완전히 안전하지만 OOM

```rust,no_run
fn main() {
    let mut cache = Vec::new();
    loop {
        cache.push(vec![0_u8; 1024 * 1024]);
    }
}
```

모든 element에는 유효한 owner가 있고 use-after-free도 없다. 그러나 cache가 bounded하지 않으므로 결국 allocation failure나 OS/cgroup OOM에 도달할 수 있다.

### 시나리오 B — `Drop`은 실행됐지만 RSS는 유지

```rust
fn main() {
    let data = vec![0_u8; 16 * 1024 * 1024];
    drop(data);
    // Vec의 allocation은 deallocation 대상이 되었다.
    // 이 시점의 RSS 감소는 Rust가 보장하지 않는다.
}
```

`drop(data)`는 소유한 `Vec`을 파괴한다. allocator가 block을 OS에 즉시 반환하는지, page가 즉시 non-resident가 되는지는 별개의 계약이다.

### 시나리오 C — `try_reserve` 성공 뒤 OOM kill

Linux overcommit 환경에서는 virtual address/commit 요청이 성공해도 실제 page를 touch하는 시점에 memory pressure가 발생할 수 있다. 따라서 `try_reserve`는 application admission이나 physical RAM reservation과 같지 않다.

## 보장 경계

### 이 장이 보장하는 설명

- sound한 safe Rust가 막는 invalid memory access 범주
- leak, growth, fragmentation, RSS, OOM이 memory safety와 별개라는 분류

### 이 장이 보장하지 않는 것

- `unsafe` code, FFI, soundness bug가 있는 dependency까지 자동으로 안전하다는 주장
- data race 방지가 모든 concurrency bug를 막는다는 주장
- `Drop`이나 `try_reserve`가 process memory 상한을 보장한다는 주장

### 출처와 권위

- **공식 학습서:** [References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html), [Reference Cycles Can Leak Memory](https://doc.rust-lang.org/book/ch15-06-reference-cycles.html)
- **언어 규범:** [Behavior considered undefined](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)
- **고급/unsafe:** [Rustonomicon — Leaking](https://doc.rust-lang.org/nomicon/leaking.html)
- **구현 확인:** [`Vec` guarantees](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees), [`handle_alloc_error`](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)
- **OS 공식:** [Linux overcommit accounting](https://www.kernel.org/doc/html/latest/mm/overcommit-accounting.html), [cgroup v2 memory controller](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory)
