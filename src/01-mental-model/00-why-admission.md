# 왜 allocation failure 대신 admission인가

이 책을 만드는 가장 직접적인 이유는 다음 차이에 있다.

> **일반적인 Rust `std` 프로그램에서는 OOM을 모든 allocation에 공통으로 적용되는 복구 가능한 exception 경계로 사용할 수 없다. 따라서 server/DB는 allocation failure가 발생하기 전에 admission으로 동시 workload를 제한해야 한다.**

Admission은 단순한 예측 기법이 아니다. 올바르게 구현하면 관리 대상 workload에 부여한 **memory commitment의 합**에 대해 정확한 invariant를 만든다. 다만 그 invariant를 process의 total RSS와 동일시해서는 안 된다.

[첫 화면의 추적 구성 비교](../introduction.md#먼저-비교하기--어디서-세고-어디서-거절하는가)는 양쪽 모두 allocation byte를 셀 수 있음을 먼저 보여준다. Rust의 관리 대상 buffer도 **다음 allocation의 Layout을 구한 뒤, 그 크기를 승인받고, 실패하면 `Result`로 돌아오는 구성**을 만들 수 있다. “Rust에서는 모두 추측해야 한다”와 “추적할 수 없다”는 결론은 맞지 않는다. [실전 정렬 예제](../08-governance/18a-deterministic-reservation.md)에서 이 구성을 확인한다.

## C++ 개발자에게 익숙한 제어 방식

C++에서는 다음 구조를 만들 수 있다.

```text
task 실행
  ↓
capped allocator / operator new
  ├─ 한도 안: pointer 반환
  └─ 한도 초과: std::bad_alloc throw
                    ↓
              task boundary에서 catch
                    ↓
              rollback / reject / backpressure
```

C++의 throwing allocation function은 storage를 제공하지 못하면 `std::bad_alloc`과 일치하는 exception으로 실패를 알린다. 모든 관련 allocation이 같은 capped allocator를 통과하고 exception-safe cleanup이 성립한다면, allocator failure를 작업 단위 복구 경계로 사용할 수 있다.

이 경우에도 두 목적은 다르다.

- **admission:** workload 분배, fairness, queue bound, latency 보호, backpressure를 결정한다.
- **allocator cap + exception:** admission의 오차나 예기치 않은 growth가 한도를 넘을 때 마지막으로 작업을 실패시킨다.

따라서 C++의 익숙한 모델은 “estimate가 정확해서”가 아니라 **estimate가 틀려도 recoverable allocation failure가 뒤에서 받쳐 주는 구조**에 가깝다.

### 이 모델이 성립하는 조건

다음 조건이 빠지면 C++에서도 allocator counter가 process memory의 확실한 상한이 되지는 않는다.

1. 관리 대상 allocation이 모두 같은 allocator 또는 arena를 통과해야 한다.
2. 직접 `mmap`, thread stack, native library와 별도 memory pool을 따로 제한해야 한다.
3. Linux overcommit 환경에서는 allocation 요청 성공과 실제 page backing을 구분해야 한다.
4. exception이 전달되는 동안 필요한 cleanup이 exception-safe해야 한다.
5. allocator가 세는 requested/live byte와 allocator metadata, fragmentation, resident page의 차이를 고려해야 한다.

특히 미리 확보하고 실제 page까지 touch한 bounded arena는 강한 상한을 만들 수 있지만, 일반 process allocator의 active requested byte counter와 RSS는 같은 값이 아니다.

## Rust의 기본 failure path가 다른 지점

Rust의 `GlobalAlloc::alloc`은 실패를 null pointer로 표현할 수 있다. 그러나 `Vec::push`, `String` growth, `Box::new`, formatting과 여러 dependency API는 allocation failure를 `Result`로 노출하지 않는 **infallible allocation model**을 사용한다.

```text
task 실행
  ↓
Vec::push / Box::new / clone / format / dependency
  ↓
GlobalAlloc이 null 반환
  ↓
handle_alloc_error
  ↓
일반적인 std 기본 구성: process abort
```

`handle_alloc_error`는 정상적으로 return하지 않는 함수다. 현재 `std`를 링크한 기본 동작은 메시지를 출력하고 process를 abort한다. 또한 `GlobalAlloc` 구현 자체가 unwind하는 것은 현재 safety contract상 undefined behavior다.

따라서 다음 C++식 전제를 일반적인 stable Rust 프로그램에 그대로 둘 수 없다.

```text
"어디선가 allocation이 한도를 넘으면
 allocator가 exception을 던지고
 query boundary에서 공통으로 catch하면 된다"
```

Rust에서 recoverable path를 만들려면 allocation 이전에 명시적으로 `try_reserve` 같은 fallible API를 호출하고, 이후 경로가 확보한 capacity 밖에서 다시 allocation하지 않는다는 구조를 만들어야 한다. Transitive dependency의 infallible allocation까지 자동으로 바뀌는 것은 아니다.

## 두 언어의 기본 경계를 정확히 비교하기

| 질문 | C++의 일반적인 throwing 경로 | Rust `std`의 기본 경로 |
|---|---|---|
| allocator가 요청을 만족하지 못하면? | throwing allocation은 `std::bad_alloc`으로 실패 가능 | `GlobalAlloc`은 null을 반환할 수 있음 |
| 일반 collection growth가 실패를 전달하는가? | exception이 call stack을 unwind할 수 있음 | infallible API는 보통 `handle_alloc_error`로 감 |
| task boundary에서 공통 복구 가능한가? | exception-safe하다는 전제에서 가능 | 기본 OOM abort에서는 불가능 |
| allocator가 직접 unwind해도 되는가? | allocation function의 exception 계약에 따름 | `GlobalAlloc` method의 unwind는 현재 undefined behavior |
| 명시적 fallible API가 있는가? | nothrow/별도 allocator API 등을 설계 가능 | collection의 `try_reserve` 계열이 일부 경로를 제공 |
| Linux overcommit을 피하는가? | 아니요 | 아니요 |

이 비교는 “C++은 항상 안전하고 Rust는 불가능하다”는 뜻이 아니다. Rust에서도 bounded arena와 fallible API를 조합할 수 있다. 차이는 **언어 ecosystem의 기본 collection failure path를 전역 exception으로 복구할 수 있느냐**에 있다.

## Allocator counter가 total memory가 아닌 이유

Allocator hook은 실제로 자신을 통과한 allocation 요청을 세고 제한할 수 있다. 그러나 그 counter는 다음 이유로 process 또는 workload의 total memory와 같지 않다.

- optimizer가 제거하거나 stack으로 옮긴 source-level allocation은 allocator 호출이 아니다. Rust의 `GlobalAlloc` 문서는 allocation 발생 자체에 program semantics를 의존하지 말라고 명시한다.
- requested size와 allocator size class, alignment, metadata, arena, fragmentation의 실제 비용은 다를 수 있다.
- allocator에 deallocation된 block도 allocator가 OS에 반환하지 않고 보관할 수 있다.
- thread stack과 직접 만든 anonymous `mmap`은 Rust global allocator 요청 byte에 포함되지 않는다.
- FFI/native library가 다른 allocator나 별도 pool을 사용할 수 있다.
- page cache, socket buffer, page table 같은 cgroup charge도 Rust heap counter 밖에 있다.

따라서 allocator counter는 **participating heap allocation의 훌륭한 관측값 또는 enforcement 지점**이지만, 그 자체로 total RSS나 cgroup consumption의 완전한 정의는 아니다.

## Anonymous memory는 왜 생기는가

Linux에서 anonymous memory는 “주인을 알 수 없는 메모리”가 아니다. **filesystem의 file로 backing되지 않은 mapping**을 뜻한다.

```text
anonymous memory
├─ program heap
├─ thread stack
├─ mmap(MAP_ANONYMOUS)
├─ allocator arena와 큰 allocation mapping
└─ MAP_PRIVATE file page를 수정해 생긴 Copy-on-Write page
```

Heap과 stack의 virtual mapping은 먼저 만들어지고, 실제 write 시점에 physical page가 연결될 수 있다. 이 때문에 allocator가 virtual allocation 성공을 반환한 시점과 `RssAnon` 또는 cgroup `anon`이 증가하는 시점도 일치하지 않을 수 있다.

Anonymous memory는 process/cgroup 수준에서는 관측할 수 있지만, “어느 query가 몇 byte를 소유하는가”라는 application 의미는 kernel이 알지 못한다. 그 attribution을 위해 application reservation과 charge가 필요하다.

## Admission이 실제로 보장하는 것

먼저 관리 대상 예산을 정의한다.

```text
L_cgroup
  - baseline_peak
  - allocator/native/runtime headroom
  - kernel-facing/file/socket headroom
  - pressure margin
= B_governed
```

각 실행 중인 작업의 reservation을 `R_i`라 하면 memory manager는 다음 invariant를 정확히 지킬 수 있다.

```text
sum(R_i) <= B_governed
```

각 작업도 관리 대상 buffer를 늘리기 전에 추가 grant를 얻도록 만든다.

```text
다음 allocation의 domain charge가 reservation을 넘는가?
  ├─ 아니요 → 기존 grant 안에서 실행
  └─ 예     → incremental reserve
                ├─ 성공 → try_reserve / grow
                └─ 실패 → wait / spill / reject / cancel
```

이 규칙을 지키면 admission은 **governed commitment의 hard bound**를 보장한다. 처음 estimate가 정확해야만 성립하는 구조가 아니다. Estimate는 초기 grant의 크기와 scheduling 효율을 결정하며, 실행 중의 incremental reservation과 charge가 correctness를 유지한다.

반대로 다음 등식은 성립하지 않는다.

```text
sum(R_i) == allocator live bytes == RssAnon == memory.current
```

Admission이 total RSS를 직접 보장하지 않는 이유는 policy가 부정확해서가 아니라, 각 숫자가 서로 다른 resource boundary를 측정하기 때문이다.

## 통제 강도의 네 단계

| 단계 | 만드는 상한 | 장점 | 남는 위험 |
|---|---|---|---|
| 시작 시 estimate만 검사 | admitted estimate 합 | 구현이 단순함 | 실행 중 under-estimation을 막지 못함 |
| reservation + charge + incremental top-up | 관리 대상 workload의 commitment | backpressure와 작업별 attribution 가능 | untracked/native/stack은 headroom 필요 |
| task-local bounded arena + fallible growth | arena가 담당하는 실제 byte | 작업별 강한 격리와 teardown | 모든 dependency를 arena에 태우기 어렵고 기본 infallible API가 남을 수 있음 |
| process/cgroup 격리 | cgroup이 account하는 memory | 우회 allocation까지 넓게 containment | `memory.max`의 최종 집행은 graceful error가 아니라 OOM kill일 수 있음 |

Server/DB의 일반적인 추천은 두 번째 단계를 기본으로 하고, 큰 query operator나 tenant에는 세 번째 단계를 선택적으로 적용하며, 네 번째 단계를 최종 containment로 두는 것이다.

## 한 문장으로 정리하기

> **C++에서는 capped allocator의 recoverable exception이 admission의 오차를 뒤에서 막아 줄 수 있다. Rust의 기본 OOM 경로는 process abort이므로, 그 방어선을 allocation 이전의 reservation과 admission으로 옮겨야 한다. Admission은 RSS를 예언하는 장치가 아니라 관리 대상 workload의 commitment를 제한하는 장치이며, 남은 anonymous/untracked memory는 headroom, 관측, arena, cgroup으로 닫는다.**

## 보장 경계

### 이 장이 보장하는 설명

- C++ throwing allocation과 Rust 기본 allocation failure path의 계약 차이
- Admission이 정확히 제한할 수 있는 대상이 physical RAM이 아니라 governed commitment라는 점
- Anonymous memory가 생기는 OS-level 이유와 allocator accounting의 가시성 한계

### 이 장이 보장하지 않는 것

- 모든 C++ 프로그램이 `std::bad_alloc`에서 안전하게 복구한다는 주장
- Rust에서 custom arena나 fallible allocation을 구현할 수 없다는 주장
- Application reservation만으로 process RSS나 cgroup usage가 완전히 제한된다는 주장

### 출처와 권위

- **C++ 비교 규범:** [C++ working draft — Dynamic storage allocation](https://eel.is/c++draft/basic.stc.dynamic.allocation), [Storage allocation errors](https://eel.is/c++draft/new.handler)
- **구현 확인/public contract:** [`GlobalAlloc` safety and errors](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html), [`handle_alloc_error`](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html), [`TryReserveError`](https://doc.rust-lang.org/std/collections/struct.TryReserveError.html)
- **설계 배경:** [RFC 2116 — Alloc Me Maybe](https://github.com/rust-lang/rfcs/blob/master/text/2116-alloc-me-maybe.md)
- **OS 공식:** [Linux memory management concepts — Anonymous Memory](https://docs.kernel.org/admin-guide/mm/concepts.html#anonymous-memory), [cgroup v2 memory controller](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)
