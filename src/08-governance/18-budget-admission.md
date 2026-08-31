# Memory budget과 admission

Server/DB memory governance의 목표는 “미래의 RSS를 정확히 맞히는 것”이 아니다. **관리 대상 workload에 부여한 memory commitment의 합을 제한하고, allocation failure가 process abort가 되기 전에 wait, spill, reject, cancellation을 선택하는 것**이다.

C++의 capped allocator와 `std::bad_alloc`처럼 모든 allocation에 공통인 recoverable fallback이 있다면 admission은 주로 workload 분배와 backpressure를 담당할 수 있다. 일반적인 Rust `std` 프로그램에서는 그 fallback을 전제로 둘 수 없으므로 admission이 survivability의 첫 번째 경계가 된다.

## Admission의 정확한 invariant

Application이 직접 통제할 예산을 `B_governed`, 실행 중인 각 작업의 reservation을 `R_i`라고 하자.

```text
sum(R_i) <= B_governed
```

`MemoryManager`의 atomic accounting은 이 식을 정확히 보장할 수 있다. Estimate가 부정확하더라도 산술 invariant 자체가 흐려지는 것은 아니다.

중요한 조건은 작업이 관리 대상 buffer를 늘리기 전에 다음 규칙을 지키는 것이다.

```text
charged_i + next_growth <= R_i
  ├─ yes → allocation 진행
  └─ no  → incremental reserve
             ├─ grant → try_reserve / grow
             └─ deny  → wait / spill / reject / cancel
```

이 규칙을 어긴 allocation은 **untracked allocation**이다. Admission의 오차가 아니라 instrumentation/architecture coverage의 빈 곳으로 다뤄야 한다.

## `B_governed`를 정하는 방법

Application budget은 cgroup limit 전체가 아니다.

```text
L_cgroup
  - baseline_peak
  - allocator metadata / fragmentation / retention headroom
  - stack / native / direct mmap headroom
  - page cache / socket / kernel-facing headroom
  - pressure margin
= B_governed
```

`baseline_peak`와 headroom은 공식 상수가 아니다. Workload stress, allocator profile, `RssAnon`, `smaps_rollup`, cgroup `memory.stat`, `memory.current`를 함께 관측해 정한다. Unknown을 모두 estimate에 억지로 넣기보다 governed budget 바깥의 명시적인 headroom으로 분리한다.

## 최소 구성 요소

```text
MemoryManager
├─ global_limit
├─ reserved_bytes
├─ tracked_used_bytes
├─ untracked_headroom
└─ pressure_state

TaskReservation (RAII)
├─ task_id
├─ granted_bytes
├─ consumed_bytes
└─ Drop → grant 반환
```

`TaskReservation`의 `Drop`은 정상 경로와 unwind에서 grant 반환을 단순화한다. 그러나 abort/OOM kill에서는 실행되지 않으므로 process-local counter 복구와 외부 durable lease 문제를 구분한다.

## 주요 data flow

```text
1. plan/입력에서 initial estimate 계산
2. MemoryManager.reserve(initial_estimate)
   ├─ grant → TaskReservation
   ├─ wait  → bounded queue
   └─ reject → RESOURCE_EXHAUSTED
3. 관리 대상 state를 grow하기 전 reservation 여유 확인
   ├─ 부족 → incremental reserve / spill / wait / fail
   └─ 충분 → collection.try_reserve(known_growth)
4. try_reserve Err → reservation 반환, task error
5. 성공한 growth를 domain charge에 반영
6. estimate와 charge, allocator/OS 관측값 reconcile
7. 관리 대상 storage의 최종 owner가 dealloc한 뒤 reservation Drop
```

## API boundary 예시

```text
trait MemoryPool {
    fn try_reserve(&self, bytes: usize) -> Result<Reservation, MemoryError>;
}

struct Reservation {
    pool: Arc<PoolInner>,
    granted: usize,
}

impl Reservation {
    fn try_grow(&mut self, additional: usize) -> Result<(), MemoryError>;
    fn shrink(&mut self, bytes: usize) -> Result<(), MemoryError>;
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.pool.release(self.granted);
    }
}
```

실제 `Reservation`에는 incremental `try_grow`와 일부 grant를 돌려주는 `shrink`가 필요할 수 있다. Integer overflow, concurrent compare-and-swap, partial growth, cancellation, hierarchical budget, double release 방지를 테스트해야 한다. 실행 가능한 최소 구현은 저장소의 `examples/memory-lab`에 있다.

## 실전 예제 1 — Request admission과 bounded backpressure

Admission에서 중요한 ownership 규칙은 **실패한 task를 잃지 않고 caller에게 돌려주는 것**이다. Caller는 동일한 task를 bounded queue에 넣거나 즉시 reject할 수 있다.

```rust,ignore
use memory_lab::{Admission, MemoryPool, admit};

let pool = MemoryPool::new(512 * 1024 * 1024);
let initial_estimate = request.estimated_build_bytes();

match admit(&pool, request, initial_estimate) {
    Admission::Admitted { task, reservation } => {
        // Reservation을 task와 함께 move한다. 결과가 남으면 grant도 함께 넘긴다.
        workers.submit(move || run_query(task, reservation));
    }
    Admission::Backpressured { task, error } => {
        // queue도 반드시 bounded여야 한다.
        if waiting_queue.try_send(task).is_err() {
            return Err(resource_exhausted(error));
        }
    }
}
```

이 예제에서 정확히 보장하는 것은 성공한 reservation의 합이 `MemoryPool::limit()`을 넘지 않는다는 점이다. Queue 안의 task에는 아직 grant가 없으므로, dequeue 후 다시 admission을 거쳐야 한다.

실행 가능한 `admit` 구현은 [`examples/memory-lab/src/lib.rs`](https://github.com/Tangeroooo/rust-memory-for-systems-engineers/blob/main/examples/memory-lab/src/lib.rs)에 있으며 다음 조건을 test한다.

- budget이 부족하면 task ownership과 error를 함께 반환
- 활성 reservation이 `Drop`된 뒤 같은 task를 다시 admit 가능
- concurrent admission에서도 `reserved <= limit`

## 입문 예제 2 — Logical byte accounting의 한계

초기 estimate만 검사해서는 실행 중 growth를 통제할 수 없다. 관리 대상 wrapper가 다음 순서를 강제해야 한다.

```rust,ignore
use memory_lab::{BudgetedBuffer, MemoryPool};

let pool = MemoryPool::new(256 * 1024 * 1024);
let mut build_side = BudgetedBuffer::new(&pool, 64 * 1024)?;

for batch in input_batches {
    // 내부 순서:
    // 1. next logical length 계산
    // 2. 부족한 grant를 Reservation::try_grow로 먼저 요청
    // 3. Vec::try_reserve로 fallible allocation 시도
    // 4. allocation 실패 시 방금 얻은 grant rollback
    // 5. 성공한 경우에만 data 추가
    build_side.try_extend(batch.as_bytes())?;
}
```

`BudgetedBuffer`는 다음 invariant를 유지한다.

```text
buffer.len() <= reservation.granted_bytes()
```

그러나 `Vec::capacity()`가 logical length보다 클 수 있으므로 다음 등식은 보장하지 않는다.

```text
reservation bytes == Vec capacity bytes == allocator usable size == RSS
```

이 예제는 **logical/domain byte accounting** 패턴일 뿐 requested allocation byte의 hard cap이 아니다. `Vec`의 spare capacity와 growth overlap이 grant 밖으로 나갈 수 있다. 이를 측정해 운영 headroom을 잡을 수는 있지만, 임의의 고정 headroom이 이 차이를 항상 덮는다는 증명은 없다.

### 실제 allocation에 연결한 다음 단계

[실습: estimate에서 deterministic 반환까지](18a-deterministic-reservation.md)는 이 한계를 보완한다. 고정 폭 row의 **capacity 전체 Layout**을 승인하고, 새 block을 만들 때는 old와 new가 공존하는 peak까지 charge한다.

```text
4 rows × 16 B = 64 B 승인
5번째 row → capacity 8로 growth
old 64 B를 유지한 채 new 128 B 추가 승인 → peak 192 B
copy → old dealloc → old grant 64 B 반환 → 128 B 유지
정렬 완료 → output에 storage와 grant를 함께 move
최종 output Drop → dealloc → 128 B 반환
```

여기서는 초기 estimate가 틀리더라도 다음 allocation의 크기를 **요청 전에 계산**한다. Unsafe 부분은 storage wrapper인 `Block`/`SortBuffer` 내부에 한정하며, 상위 operator는 fallible `push`와 ownership API를 사용한다. Bounded arena만이 유일한 선택은 아니다. 명시적 Layout allocation wrapper로도 관리 대상 requested byte의 cap을 만들 수 있다.

## 구현 규칙 checklist

1. Task를 worker에 보내기 전에 initial reservation을 얻는다.
2. Reservation의 ownership을 task와 함께 이동한다.
3. 관리 대상 state는 allocation 전에 추가 grant를 얻는다.
4. Fallible allocation 실패 시 추가 grant를 rollback한다.
5. Backpressure queue와 spill buffer 자체도 bounded하게 만든다.
6. 정상 완료, cancellation, early return에서 **storage가 소멸할 때** RAII `Drop`으로 grant를 반환한다. 결과가 살아 있으면 grant도 그 결과와 함께 이동한다.
7. Abort/OOM kill에서는 `Drop`이 실행되지 않는다는 전제로 process restart와 외부 state recovery를 설계한다.

## Estimate와 accounting

- **estimate:** admission 전 미래 사용량의 예측
- **reservation:** 해당 작업에 부여한 논리적 권한
- **charge:** 실제 allocation 또는 domain object 생성에 따라 기록한 사용량
- **reconciliation:** estimate와 실제 관측의 차이를 보정

Estimate는 admission correctness가 아니라 **utilization과 scheduling 품질**을 좌우한다.

- 너무 크게 잡으면 안전성은 유지되지만 concurrency와 utilization이 낮아진다.
- 너무 작게 잡으면 작업이 실행 중 추가 grant를 자주 요청하고 spill/reject가 늘어난다.
- 추가 grant 없이 reservation을 초과해 allocation하면 invariant가 깨진다. 이는 estimation error가 아니라 governance contract 위반이다.

Conservative upper bound만 고집하면 memory를 낭비할 수 있으므로 initial estimate, incremental reservation, workload class별 calibration을 조합한다.

## 두 개의 ledger

운영에서는 다음 두 ledger를 섞지 않는다.

| ledger | 질문 | 대표 값 |
|---|---|---|
| **commitment ledger** | 누가 얼마를 사용할 권한이 있는가? | reservation, charge, queue, spill threshold |
| **consumption ledger** | process/cgroup가 실제로 얼마를 account하는가? | allocator active/retained, `RssAnon`, `memory.current`, `memory.stat` |

Commitment ledger는 task attribution과 backpressure를 가능하게 한다. Consumption ledger는 untracked growth와 headroom 부족을 발견한다. 두 값의 차이를 주기적으로 reconciliation해야 하지만 동일하게 만들 수 있다고 가정하지 않는다.

## 통제 방식 비교

| 방식 | 보장 | 장점 | 위험한 edge case |
|---|---|---|---|
| 시작 시 estimate만 검사 | admitted estimate 합 | 단순함 | 실제 growth가 estimate를 넘음 |
| incremental reservation + charge | governed commitment 상한 | workload별 backpressure와 attribution | 모든 grow path가 contract를 따라야 함 |
| bounded task arena + fallible path | arena가 담당하는 byte 상한 | 더 강한 task isolation | transitive infallible allocation과 arena 밖 memory |
| cgroup/process 격리 | 넓은 system accounting의 hard containment | direct mmap/native도 넓게 포함 | 최종 enforcement가 graceful error가 아니라 OOM kill |

일반적인 server/DB에는 incremental reservation을 기본으로 추천한다. 크고 위험한 operator에는 bounded arena를 추가하고, cgroup은 마지막 crash containment로 둔다.

## 보장 경계

### 이 장이 보장하는 설명

- application budget, reservation, collection allocation의 책임 분리
- RAII reservation이 정상 경로의 release를 단순화한다는 점
- `sum(reservations) <= B_governed`라는 commitment invariant와 total RSS 상한의 차이

### 이 장이 보장하지 않는 것

- reservation byte가 physical RAM을 실제로 봉인한다는 주장
- application이 모든 dependency/native allocation을 자동 추적한다는 주장
- `Drop`만으로 kill/abort 이후 외부 state가 복구된다는 주장
- initial estimate 하나만으로 실행 중의 모든 growth가 제한된다는 주장

### 출처와 권위

- **설계 배경:** [RFC 2116 — runtime/database profile](https://github.com/rust-lang/rfcs/blob/master/text/2116-alloc-me-maybe.md#runtime-developer)
- **구현 확인:** [`TryReserveError`](https://doc.rust-lang.org/std/collections/struct.TryReserveError.html)
- **구현 확인/safety contract:** [`GlobalAlloc`](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html), [`handle_alloc_error`](https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html)
- **OS 공식:** [cgroup v2 memory controller](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory)
- **이 책의 권고:** `MemoryManager`/`Reservation` interface는 표준 Rust API가 아니라 server/DB 설계 예시다.
- **검증된 예제:** [`memory-lab`](https://github.com/Tangeroooo/rust-memory-for-systems-engineers/tree/main/examples/memory-lab)
