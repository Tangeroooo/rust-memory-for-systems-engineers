# 들어가며

Rust의 메모리 관리를 `ownership` 하나로 설명하면 중요한 절반을 놓친다. ownership은 누가 값을 소유하고 누가 접근할 수 있는지를 결정한다. 그러나 장시간 실행되는 서버가 memory limit 안에서 살아남는지는 collection의 capacity, allocator의 재사용 정책, Linux의 virtual memory, cgroup, 애플리케이션의 admission policy까지 함께 결정한다.

## 먼저 비교하기 — 어디서 세고, 어디서 거절하는가

아래는 **관리 대상 allocation을 연결해 놓은 C++ 구성**과 **명시적인 fallible 경로를 만든 Rust 구성**의 비교다. 두 언어의 기본 설정을 비교하는 그림이 아니다. C++의 `std::pmr::memory_resource`도 자동으로 cap을 제공하지 않으며, Rust의 `ownership`도 자동으로 budget을 제공하지 않는다.

<iframe class="memory-diagram memory-diagram--wide memory-diagram--comparison" src="assets/diagrams/tracking-comparison.html" title="C++와 Rust의 메모리 영역·추적 범위"></iframe>

[비교 다이어그램을 크게 열기](assets/diagrams/tracking-comparison.html). 좁은 화면에서는 그림을 가로로 스크롤할 수 있다.

그림은 호출 순서가 아니라 **process 안의 메모리 영역과 accounting 경계의 포함 관계**를 보여준다. 큰 면 안에 allocator의 backing 영역이 있고, 그 안의 강조한 면이 task에 charge한 storage다. 사용 중인 slot뿐 아니라 여유 capacity도 이 면에 포함된다. Stack과 직접 `mmap`처럼 선택한 allocator를 통과하지 않는 영역은 아래에 분리했다. 면적은 byte 비율이 아니며, 실행 코드와 file-backed mapping 등은 비교의 초점을 위해 생략했다. Reservation 자체가 물리적인 메모리 영역이라는 뜻도 아니다.

그림에서 읽어야 할 차이는 세 가지다.

1. **추적 가능성은 공통이다.** C++도 Rust도 자신을 통과한 allocator 요청을 셀 수 있다. Allocator-aware container 안에 일반 `std::string`을 넣거나, Rust wrapper 안에서 별도의 `String`을 만들면 nested allocation은 선택한 budget 밖으로 나갈 수 있다.
2. **실패 전달의 기본값이 다르다.** C++의 capped throwing allocator는 `std::bad_alloc`으로 task 경계까지 돌아올 수 있다. Rust는 명시적 `Result` 경로가 필요하며, 보통의 infallible allocation은 기본 `std` 구성에서 OOM abort로 이어진다.
3. **두 숫자는 분리한다.** Requested allocation byte의 상한과 total RSS의 상한은 다르다. 각 process 하단의 stack·직접 `mmap`·다른 allocator뿐 아니라, 회색으로 표시한 allocator metadata와 retention도 별도로 고려한다.

따라서 Rust에서 모든 메모리를 미리 정확히 예언해야 하는 것은 아니다. **초기 estimate는 실행 계획을 정하고, 다음 allocation의 크기가 확정되는 시점에 budget을 집행한다.** 이 책의 [정렬 buffer 실습](08-governance/18a-deterministic-reservation.md)은 `4 rows × 16 B = 64 B`에서 시작해 growth, rollback, 결과 소유권과 최종 반환까지 그 규칙을 코드로 따라간다.

위 비교는 [C++ memory_resource 계약](https://eel.is/c++draft/mem.res.private), [Rust GlobalAlloc 계약](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html), [기본 allocation error 처리](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)에 근거한다. 구체적인 reservation wrapper는 이 책의 설계 예시이며 Rust 언어 자체의 기능은 아니다.

이 책은 다음 문장을 출발점으로 삼는다.

> **Rust의 ownership은 메모리 사용량을 관리하는 체계가 아니라, 값의 lifetime과 접근 권한을 관리하는 체계다.**

따라서 다음 두 질문은 분리해야 한다.

- **memory safety:** 이 reference를 역참조해도 유효한가? 같은 메모리에 충돌하는 접근이 있는가? 누가 `Drop`할 것인가?
- **memory capacity:** 이 작업이 몇 byte를 필요로 하는가? process와 cgroup의 여유는 얼마인가? 초과하면 대기, spill, 실패 중 무엇을 할 것인가?

## 왜 allocation failure까지 기다릴 수 없는가

C++에서 capped allocator와 `std::bad_alloc`을 task boundary에 연결해 본 개발자라면 allocation failure를 마지막 recoverable guard로 생각하기 쉽다. Admission은 workload 분배와 backpressure를 담당하고, estimate가 틀리면 allocator exception이 작업만 실패시키는 구조다.

일반적인 Rust `std` 프로그램에서는 같은 전제를 둘 수 없다. Infallible collection allocation이 실패하면 기본적으로 process abort로 이어지며, `GlobalAlloc` 구현도 unwind할 수 없다. `try_reserve`는 일부 collection growth를 fallible하게 만들지만 dependency와 이후의 모든 allocation을 자동으로 포함하지 않는다.

따라서 이 책의 핵심 질문은 다음과 같다.

> **Recoverable global OOM boundary가 없는 Rust server/DB에서, process를 죽이지 않고 workload를 어떻게 제한할 것인가?**

답은 “초기 estimate를 완벽하게 맞힌다”가 아니다. 관리 대상 작업에 reservation을 부여하고, grow 전에 incremental grant를 얻도록 하여 commitment의 합을 제한한다. Total RSS에 포함되지만 이 accounting 밖에 있는 anonymous/native/kernel-facing memory는 별도 headroom과 cgroup으로 통제한다.

이 차이는 [왜 allocation failure 대신 admission인가](01-mental-model/00-why-admission.md)에서 먼저 자세히 설명한다.

## 읽는 순서

```text
값과 접근 권한
    ↓
객체가 파괴되는 시점
    ↓
heap allocation을 소유하는 type
    ↓
allocator의 allocate/deallocate
    ↓
OS의 virtual/physical memory
    ↓
cgroup의 reclaim/OOM
    ↓
server/DB의 memory governance
```

C/C++ 개발자는 RAII와 smart pointer라는 익숙한 발판을 이용하되, 대응 관계를 동일성으로 오해하지 않아야 한다. 예를 들어 `Box<T>`와 `std::unique_ptr<T>`는 단일 소유권이라는 점에서 비슷하지만, Rust의 move와 borrow checking은 C++의 move constructor나 관례적 pointer discipline보다 언어 전반에 더 강하게 연결된다.

## 이 책이 다루지 않는 것

- `unsafe` Rust 전체를 가르치는 완전한 지침
- 특정 allocator가 모든 workload에서 우수하다는 성능 결론
- 모든 Linux 배포판과 kernel version에 동일하게 적용되는 tuning 값
- 어떤 DB 엔진에도 그대로 복사할 수 있는 단일 memory pool 구현

이 책은 layer 사이의 계약과 실패 경로를 설명한다. 실제 한도와 정책 값은 workload 측정으로 결정해야 한다.

## 보장 경계

### 이 장이 보장하는 설명

- memory safety와 memory capacity가 서로 다른 문제라는 분류
- 뒤 장에서 사용할 layer와 용어의 일관된 순서

### 이 장이 보장하지 않는 것

- safe Rust 프로그램이 leak이나 OOM을 일으키지 않는다는 주장
- allocation 성공이 physical memory 확보를 뜻한다는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — What Is Ownership?](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
- **언어 규범:** [Rust Reference — Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **C++ 비교 규범:** [C++ working draft — memory_resource](https://eel.is/c++draft/mem.res.private)
- **Standard library 계약:** [`GlobalAlloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html), [`handle_alloc_error`](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)
- **OS 공식:** [Linux kernel — Overcommit Accounting](https://www.kernel.org/doc/html/latest/mm/overcommit-accounting.html)
