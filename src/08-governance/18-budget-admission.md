# Memory budget과 admission

Server/DB memory governance의 목표는 allocator failure가 발생한 뒤에만 반응하는 것이 아니다. 작업이 시작되기 전에 **누가 얼마를 사용할 권한이 있는지** 결정하고, 실행 중 실제 사용량을 추적하며, 압박 시 대체 경로를 선택하는 것이다.

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
1. plan/입력에서 memory estimate 계산
2. MemoryManager.reserve(estimate)
   ├─ grant → TaskReservation
   ├─ wait  → bounded queue
   └─ reject → RESOURCE_EXHAUSTED
3. collection.try_reserve(known_upper_bound)
   ├─ Err → reservation 반환, 작업 실패
   └─ Ok  → 실행
4. 실제 사용량 charge/reconcile
5. threshold 초과 시 spill/evict/backpressure
6. 완료/취소 시 reservation Drop
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

impl Drop for Reservation {
    fn drop(&mut self) {
        self.pool.release(self.granted);
    }
}
```

실제 구현에서는 integer overflow, concurrent compare-and-swap, partial growth, cancellation, hierarchical budget, double release 방지를 테스트해야 한다. 실행 가능한 최소 구현은 저장소의 `examples/memory-lab`에 있다.

## Estimate와 accounting

- **estimate:** admission 전 미래 사용량의 예측
- **reservation:** 해당 작업에 부여한 논리적 권한
- **charge:** 실제 allocation 또는 domain object 생성에 따라 기록한 사용량
- **reconciliation:** estimate와 실제 관측의 차이를 보정

Estimate가 부정확하다는 이유로 admission을 포기하지 않는다. Conservative upper bound, incremental reservation, spill threshold, workload class별 calibration을 조합한다.

## 보장 경계

### 이 장이 보장하는 설명

- application budget, reservation, collection allocation의 책임 분리
- RAII reservation이 정상 경로의 release를 단순화한다는 점

### 이 장이 보장하지 않는 것

- reservation byte가 physical RAM을 실제로 봉인한다는 주장
- application이 모든 dependency/native allocation을 자동 추적한다는 주장
- `Drop`만으로 kill/abort 이후 외부 state가 복구된다는 주장

### 출처와 권위

- **설계 배경:** [RFC 2116 — runtime/database profile](https://github.com/rust-lang/rfcs/blob/master/text/2116-alloc-me-maybe.md#runtime-developer)
- **구현 확인:** [`TryReserveError`](https://doc.rust-lang.org/std/collections/struct.TryReserveError.html)
- **OS 공식:** [cgroup v2 memory controller](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory)
- **이 책의 권고:** `MemoryManager`/`Reservation` interface는 표준 Rust API가 아니라 server/DB 설계 예시다.
