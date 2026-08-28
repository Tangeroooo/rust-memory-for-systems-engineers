# Overcommit, page fault, OOM

Linux에서는 allocation 요청의 성공과 미래의 physical memory 가용성이 분리될 수 있다. 이 사실이 `try_reserve`를 memory admission으로 사용할 수 없는 가장 중요한 이유다.

## Overcommit mode

Linux kernel 문서는 `vm.overcommit_memory`의 세 mode를 설명한다.

| mode | 개요 |
|---:|---|
| `0` | heuristic overcommit. 명백히 과도한 요청은 거절하면서 일반 workload의 overcommit을 허용하는 기본 정책 |
| `1` | 항상 overcommit한다고 취급 |
| `2` | commit limit를 넘는 요청을 거절하는 방향의 정책 |

Mode 2도 application 단위 memory governance를 대신하지 않는다. Stack growth, shared mapping, 다른 process, cgroup limit, allocator overhead 등 전체 환경을 고려해야 한다.

## `try_reserve` 성공 뒤의 흐름

<iframe class="memory-diagram memory-diagram--wide" src="../assets/diagrams/allocation-failure.html" title="Application budget에서 cgroup OOM까지의 흐름" loading="lazy"></iframe>

[다이어그램을 새 창 크기로 보기](../assets/diagrams/allocation-failure.html)

```text
Vec::try_reserve
    ↓
allocator가 virtual memory 요청
    ↓
성공 반환
    ↓
application이 page를 실제로 write
    ↓
page fault / memory charge
    ↓
reclaim 또는 physical allocation
    ├─ 성공
    └─ 압박 심화 → global/cgroup OOM 경로
```

`try_reserve` 성공은 collection이 필요한 capacity를 확보했다는 API-level 결과다. “그만큼의 physical RAM을 이 process 전용으로 봉인했다”는 결과가 아니다.

## OOM을 한 종류로 부르지 않는다

- **allocator-visible failure:** allocation API가 failure를 반환할 수 있는 시점
- **infallible allocation abort:** Rust 일반 allocation 경로가 `handle_alloc_error`로 종료
- **global OOM:** system 전체 pressure에서 kernel OOM killer가 victim을 선택
- **cgroup OOM:** memory cgroup의 hard limit 안에서 reclaim 실패 후 victim 종료

운영 로그에는 exit code만 남기지 말고 kernel/cgroup event와 application error path를 구분해 수집한다.

## 보장 경계

### 이 장이 보장하는 설명

- Linux overcommit에서 allocation 성공과 page residency가 분리될 수 있다는 점
- allocator failure와 kernel OOM kill이 서로 다른 failure path라는 점

### 이 장이 보장하지 않는 것

- overcommit mode 2가 모든 workload에서 OOM kill을 없앤다는 주장
- `try_reserve`가 physical memory 또는 cgroup budget을 예약한다는 주장
- 특정 process가 언제 OOM victim이 될지 Rust가 제어할 수 있다는 주장

### 출처와 권위

- **OS 공식:** [Linux Overcommit Accounting](https://www.kernel.org/doc/html/latest/mm/overcommit-accounting.html), [`vm.overcommit_memory`](https://www.kernel.org/doc/html/latest/admin-guide/sysctl/vm.html#overcommit-memory)
- **설계 배경:** [RFC 2116 — Overcommit](https://github.com/rust-lang/rfcs/blob/master/text/2116-alloc-me-maybe.md#overcommit)
