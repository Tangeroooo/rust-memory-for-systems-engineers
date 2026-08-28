# cgroup v2와 OOM kill

cgroup은 Rust process의 바깥에서 resource 사용을 account하고 제한한다. Application memory pool이 협력적 제어라면 cgroup은 kernel이 집행하는 최종 containment다.

## 주요 interface

| 파일 | 의미 | 운영 해석 |
|---|---|---|
| `memory.current` | cgroup과 descendant의 현재 memory 사용량 | process RSS와 정의가 같다고 가정하지 않는다. |
| `memory.high` | 초과 시 heavy reclaim pressure와 throttle | 초과 자체가 OOM killer를 호출하지 않는 soft boundary 역할 |
| `memory.max` | hard limit | reclaim으로 낮출 수 없으면 cgroup OOM killer 경로 |
| `memory.events` | `high`, `max`, `oom`, `oom_kill` 등 event count | restart 사이에서도 수집해 원인을 분류 |
| `memory.pressure` | PSI 기반 memory pressure | 단순 사용량과 stall 영향을 함께 관찰 |

`memory.high`는 reclaim과 throttle을 유도하는 운영 pressure boundary다. `memory.max`는 cgroup memory usage의 hard limit이며 reclaim으로 낮출 수 없으면 OOM killer가 집행한다. 일부 조건에서는 일시적으로 limit을 초과할 수 있다.

Kernel 문서는 cgroup memory controller가 anonymous memory와 page cache뿐 아니라 kernel data structure와 TCP socket buffer 같은 주요 사용량도 추적한다고 설명한다. 동시에 coverage가 완전히 water-tight하지는 않다고 명시한다. 이 범위는 Rust allocator counter보다 넓지만, task/query별 attribution을 제공하지는 않는다.

## 권장 관계

```text
cgroup memory.max
  └─ crash containment을 위한 최종 상한; 초과 시 graceful error가 아니라 kill 가능

application governed budget
  └─ memory.max보다 낮음: untracked/native/kernel-facing overhead 여유

admission threshold
  └─ hard budget보다 낮음: 동시 작업과 추정 오차 흡수
```

Application budget을 `memory.max`와 같은 값으로 두면 allocator metadata, runtime stack, network buffer, page cache charge, untracked dependency allocation을 위한 여유가 없다.

## OOM 시나리오

```text
request admission 성공
  ↓
estimate보다 실제 allocation 증가
  ↓
memory.high 초과 → reclaim/throttle → latency 상승
  ↓
memory.max 근접
  ↓ reclaim 실패
cgroup OOM kill
```

이 경우 Rust destructor는 실행되지 않는다. Reservation counter가 process memory 안에만 있다면 restart로 초기화되지만, 외부 lease나 durable state가 있다면 crash recovery를 별도로 설계해야 한다.

## 보장 경계

### 이 장이 보장하는 설명

- cgroup v2의 `memory.high`와 `memory.max`가 다른 제어 경계라는 점
- `memory.events`와 pressure signal을 함께 관찰해야 한다는 점
- `memory.max`는 넓은 범위를 강제로 containment하지만 task-level error를 보장하지 않는다는 점

### 이 장이 보장하지 않는 것

- `memory.high`가 절대로 초과되지 않는 hard limit이라는 주장
- `memory.max`가 항상 정확히 그 byte에서 동기적으로 process를 종료한다는 주장
- cgroup 값과 한 process의 RSS가 동일하다는 주장

### 출처와 권위

- **OS 공식:** [Linux kernel — Control Group v2, Memory](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory), [PSI](https://www.kernel.org/doc/html/latest/accounting/psi.html)
