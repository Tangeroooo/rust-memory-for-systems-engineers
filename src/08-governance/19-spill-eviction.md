# Spill, eviction, backpressure

Admission만으로 모든 작업의 memory upper bound를 정확히 예측할 수 없다. Runtime은 pressure가 증가할 때 memory를 줄이거나 신규 work를 늦추는 escape route를 가져야 한다.

## 제어 수단 비교

| 수단 | 적합한 대상 | 장점 | 위험한 edge case |
|---|---|---|---|
| reject | 시작 전 큰 request/query | 상태가 단순하고 빠름 | estimate 오차로 처리 가능한 작업도 거절할 수 있음 |
| wait/backpressure | 짧은 pressure, bounded queue | overload를 흡수 | queue 자체가 unbounded면 memory 문제를 이동시킬 뿐 |
| spill | sort, hash aggregation/join 등 외부화 가능한 state | 큰 작업을 계속 실행 가능 | disk full, I/O amplification, cleanup, security |
| eviction | 재계산 가능한 cache | 빠르게 reclaim 가능 | hit rate 급락과 thundering herd |
| partial result / degradation | best-effort analytics/search | availability 유지 | API semantics와 사용자 기대를 명확히 해야 함 |
| cancellation | 낮은 priority 또는 deadline 초과 task | 즉시 pressure 완화 가능 | cooperative cancellation이 늦거나 cleanup이 allocation할 수 있음 |

## Pressure state machine

```text
NORMAL
  ├─ soft threshold → PRESSURED
  │                    ├─ backpressure
  │                    ├─ cache eviction
  │                    └─ early spill
  ├─ hard threshold → SHEDDING
  │                    ├─ reject new work
  │                    └─ cancel low-priority work
  └─ cgroup high/max → EMERGENCY
                       └─ survival path; allocation 최소화
```

Emergency handler가 log formatting, heap-growing metrics label, large error body를 새로 allocation하면 실패를 악화시킬 수 있다. Pressure path는 작고 반복 가능하게 테스트한다.

## Spill도 memory를 사용한다

Serialization buffer, compression workspace, I/O queue, file metadata는 allocation을 필요로 한다. Hard limit 직전에 spill을 시작하면 escape path 자체가 실패할 수 있다. Early spill threshold와 작은 reserved emergency budget을 둔다.

## 보장 경계

### 이 장이 보장하는 설명

- pressure에 대응하는 수단들의 책임과 trade-off
- backpressure queue와 spill path도 bounded되어야 한다는 점

### 이 장이 보장하지 않는 것

- 모든 algorithm이 spill 가능하다는 주장
- eviction이 즉시 RSS를 같은 양만큼 낮춘다는 주장
- cancellation 요청이 곧바로 모든 allocation을 해제한다는 주장

### 출처와 권위

- **OS 공식:** [cgroup v2 memory controller](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory)
- **이 책의 권고:** pressure state와 spill/eviction 우선순위는 application policy이며 workload 실험으로 조정해야 한다.
