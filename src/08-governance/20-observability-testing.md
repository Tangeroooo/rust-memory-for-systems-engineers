# 관측과 검증 전략

Memory system은 한 metric으로 검증할 수 없다. Language-level test, object graph, allocator profile, OS/cgroup signal, workload outcome을 계층별로 수집한다.

## 계층별 관측표

| 계층 | 관측 대상 | 대표 질문 |
|---|---|---|
| Application | task/query reservation, cache entries, queue depth | 누가 memory를 보유하는가? |
| Collection | `len`, `capacity`, nested allocation estimate | live data와 spare capacity는 얼마인가? |
| Allocator | active/allocated/retained, allocation rate | free 이후 block이 어디에 남는가? |
| Process | `VmRSS`, `RssAnon`, `smaps_rollup` | resident page 구성은 무엇인가? |
| cgroup | `memory.current`, `memory.events`, PSI | reclaim/throttle/OOM이 있었는가? |
| Service | latency, reject, spill bytes, cancellation | memory policy가 사용자 결과에 어떤 영향을 주는가? |

## 테스트 pyramid

### 1. Unit test

- reservation acquire/release와 integer overflow
- cancellation과 early return에서 RAII guard 반환
- cache/queue hard bound
- `try_reserve` error propagation

### 2. Deterministic failure injection

- allocation failure를 N번째 요청에 주입
- 반드시 별도 process에서 infallible allocation abort path 검증
- error handling이 추가 대규모 allocation을 하지 않는지 확인
- `GlobalAlloc` 구현 자체는 unwind하면 안 된다는 safety contract 준수

### 3. Workload/stress test

- burst size와 concurrency를 변화
- steady state와 post-burst idle 구간을 분리
- object count, allocator active/retained, RSS를 동시에 기록
- 반복 후 plateau인지 지속 성장인지 확인

### 4. cgroup integration test

- test 전용 cgroup/container에서 `memory.high`와 `memory.max` 설정
- pressure, throttle, OOM event를 기록
- production host나 개발자 전체 session을 위험하게 만들지 않도록 격리

## 잘못된 측정의 예

- custom allocator의 side effect만 보고 “source의 allocation은 항상 실행된다”고 가정
- RSS 하나만 보고 leak을 확정
- test input이 너무 작아 `Vec` growth가 한 번도 일어나지 않음
- debug/release, allocator, thread count가 다른 결과를 직접 비교
- cgroup OOM kill을 Rust panic으로 오인

## 완료 조건

```text
정확성: reservation 누수와 double release가 없음
경계: queue/cache/concurrency가 설정된 상한을 지킴
복구: fallible allocation 실패가 task error로 전달됨
압박: memory.high 이전에 backpressure/spill 신호가 작동
격리: memory.max 도달 시 원인이 event/log에 남음
안정성: 반복 burst 뒤 live state가 baseline 또는 설명 가능한 plateau로 복귀
```

## 보장 경계

### 이 장이 보장하는 설명

- 서로 다른 계층의 metric을 함께 관찰해야 한다는 검증 전략
- failure injection과 cgroup test를 격리해야 하는 이유

### 이 장이 보장하지 않는 것

- allocator hook의 호출 횟수가 Rust program semantics라는 주장
- 한 번의 정상 stress test가 모든 OOM 경로를 증명한다는 주장
- RSS plateau가 자동으로 memory budget 적합성을 뜻한다는 주장

### 출처와 권위

- **구현 확인:** [`GlobalAlloc` safety](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html#safety)
- **OS 공식:** [cgroup v2 memory events](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory), [Linux PSI](https://www.kernel.org/doc/html/latest/accounting/psi.html)
- **보조 학습:** [Rust Performance Book — Profiling](https://nnethercote.github.io/perf-book/profiling.html), [Heap Allocations](https://nnethercote.github.io/perf-book/heap-allocations.html)
