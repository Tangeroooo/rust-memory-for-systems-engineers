# 들어가며

Rust의 메모리 관리를 `ownership` 하나로 설명하면 중요한 절반을 놓친다. ownership은 누가 값을 소유하고 누가 접근할 수 있는지를 결정한다. 그러나 장시간 실행되는 서버가 memory limit 안에서 살아남는지는 collection의 capacity, allocator의 재사용 정책, Linux의 virtual memory, cgroup, 애플리케이션의 admission policy까지 함께 결정한다.

## 먼저 비교하기 — 어디서 세고, 어디서 거절하는가

아래는 **전용 allocator에 자체 컴포넌트와 allocator-aware library를 연결한 C++ 구성**과 **명시적인 reservation 경로와 외부 crate의 infallible allocation이 섞인 Rust 구성**의 비교다. C++의 장점은 연결된 allocation을 한 지점에서 계측하고, 한도 초과를 `std::bad_alloc`으로 전달할 수 있다는 것이다. Rust에서 어려운 부분은 자체 buffer에 만든 reservation 규칙이 외부 crate 내부의 allocation까지 자동으로 적용되지 않는다는 것이다.

<iframe class="memory-diagram memory-diagram--wide memory-diagram--comparison" src="assets/diagrams/tracking-comparison.html" title="C++와 Rust의 메모리 영역·추적 범위"></iframe>

[비교 다이어그램을 크게 열기](assets/diagrams/tracking-comparison.html). 좁은 화면에서는 그림을 가로로 스크롤할 수 있다.

그림은 호출 순서가 아니라 **allocation이 어느 계측·통제 경계에 속하는지**를 면의 포함 관계로 보여준다. 검은 테두리 안은 관리 규칙을 연결한 영역이고, Rust의 붉은 영역은 allocator에서 관측할 수 있어도 task reservation에는 연결되지 않은 allocation이다. 면적은 실제 byte 비율이나 address layout이 아니다. 사용 중인 element뿐 아니라 여유 capacity도 allocation에 포함되며, 실행 코드와 file-backed mapping 등은 생략했다.

### C++: 연결하면 allocation/deallocation이 metric을 갱신한다

컴포넌트별 counter를 가진 allocator/resource를 연결하고 상위 cap을 공유하도록 구현하면, 그 경로의 allocation과 deallocation이 metric을 갱신한다. 호출부마다 예상 byte를 따로 더하고 빼지 않아도 된다. **한도 초과 시 throw하고 task 경계에서 catch하는 계약**까지 연결하면, estimate가 틀리거나 library 내부에서 예상 밖 growth가 발생해도 작업 단위로 실패를 처리할 수 있다.

여기서 “자동”은 **allocator를 연결한 이후의 계측**을 말한다. `std::pmr::memory_resource` 자체가 counter나 cap을 제공하는 것은 아니다. 일반 `std::string`의 nested allocation, resource를 받지 않는 library, 별도 pool 등은 통제 밖에 남을 수 있다. Exception이 `noexcept` 경계를 통과하거나 cleanup이 exception-safe하지 않은 구성도 그림의 task 복구 전제에 해당하지 않는다. [C++ memory_resource 계약](https://eel.is/c++draft/mem.res.private)

### Rust: allocator에서 보이는 것과 reservation에 잡히는 것은 다르다

그림의 **Reserve는 application memory budget의 reservation**이다. `Vec::reserve`의 capacity 확보와 같은 개념이 아니다. 관리 대상 buffer는 grant를 얻고 fallible하게 grow하도록 구현하지만, 외부 crate가 내부에서 만드는 `Vec`, `String`은 그 grant를 알지 못한다. `Result`를 반환하는 API라는 이유만으로 내부 OOM도 `Err`로 돌아오는 것은 아니다.

Rust에서도 계측용 `GlobalAlloc`을 구현하면 **실제로 자신을 통과한 allocation 요청의 합계**를 셀 수 있다. 다만 합계 관측, task별 charge, recoverable failure는 각각 다른 기능이다. Global cap이 null을 반환하더라도 이를 호출한 쪽이 infallible 경로라면, 기본 `std` 구성에서는 task error 대신 process abort로 이어진다. [GlobalAlloc 계약](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html), [기본 allocation error 처리](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)

따라서 “다음 allocation의 크기를 계산해 정확히 승인한다”는 규칙은 **그 규칙에 연결한 경로 안에서만** 성립한다. [정렬 buffer 실습](08-governance/18a-deterministic-reservation.md)은 이 통제 가능한 부분을 구현한다. 외부 crate가 섞인 실제 사례와 대응은 [혼합 allocation 경로](01-mental-model/00-why-admission.md#실제-library를-넣으면-드러나는-차이)에서 구분한다.

### Headroom: 계측 밖의 비용을 위한 여유이지, 자동 방어선은 아니다

하단 막대는 **예산 배분**이며 실제 memory mapping이나 현재 사용량이 아니다. 관리 예산을 먼저 정하고, 계측 밖 allocation, allocator 간접비용·retention, runtime·stack·native 비용 등에 쓸 별도 여유인 headroom을 둔다. Baseline과 여유의 세부 구분은 [예산 산식](01-mental-model/00-why-admission.md#admission이-실제로-보장하는-것)에서 다룬다. 그림의 두 막대에 같은 비율을 사용한 것은 비교를 위한 배치이며 권장 비율이 아니다.

**C++도 headroom이 필요하다. Rust에서는 reservation에 연결되지 않은 외부 crate의 peak도 여기에 영향을 준다.** Headroom이 그 allocation을 자동으로 계측하거나 초과를 거절하지는 않는다. 따라서 입력 크기·동시 실행 수 제한과 실제 peak 관측이 함께 필요하며, 상한을 설명할 수 없는 경로는 library 교체나 별도 process 격리까지 검토해야 한다. Requested allocation byte, total RSS, cgroup consumption은 같은 숫자가 아니다.

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

답은 “초기 estimate를 완벽하게 맞힌다”가 아니다. 연결한 관리 대상 경로에서는 grow 전에 incremental grant를 얻도록 하여 commitment의 합을 제한한다. 연결하지 못한 경로에는 입력·concurrency 제한, headroom과 pressure 관측을 적용한다. Anonymous memory는 미추적 메모리의 동의어가 아니며, heap처럼 이미 계측한 부분도 포함한다. Total RSS와 cgroup consumption에는 application accounting 밖의 비용도 남으므로 최종 containment가 별도로 필요하다.

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
- 모든 외부 crate의 allocation이 application reservation에 자동 연결된다는 주장
- headroom을 설정하면 계측 밖 allocation의 상한이 집행된다는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — What Is Ownership?](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
- **언어 규범:** [Rust Reference — Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **C++ 비교 규범:** [C++ working draft — memory_resource](https://eel.is/c++draft/mem.res.private)
- **Standard library 계약:** [`GlobalAlloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html), [`handle_alloc_error`](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)
- **OS 공식:** [Linux kernel — Overcommit Accounting](https://www.kernel.org/doc/html/latest/mm/overcommit-accounting.html)
