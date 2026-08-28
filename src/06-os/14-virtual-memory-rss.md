# Virtual memory, page, RSS

User-space allocator가 다루는 pointer는 일반적인 Linux process에서 virtual address다. Virtual address space, committed memory, resident physical page는 같은 숫자가 아니다.

## Mapping과 residency

```text
virtual address range 생성/확장
          ↓
page table에 mapping 정보 존재
          ↓ first access
page fault
          ↓
physical page 연결 또는 file page 읽기
          ↓
resident set에 반영
```

모든 mapping이 즉시 physical RAM을 같은 크기로 소비하는 것은 아니다. 반대로 RSS에는 Rust heap 이외에 stack, shared library, anonymous mapping, file-backed page, shared memory가 포함될 수 있다.

## 주요 관측값

| 값 | 질문 | 주의점 |
|---|---|---|
| virtual size / `VmSize` | process가 가진 virtual address range는 얼마인가? | physical RAM 사용량과 같지 않다. |
| `VmRSS` | 현재 resident한 page의 근사 합은 얼마인가? | kernel 문서는 일부 `/proc/<pid>/status` 값이 scalability 최적화로 부정확할 수 있다고 설명한다. |
| `RssAnon` | resident anonymous memory는 얼마인가? | heap만을 뜻하지 않는다. |
| `RssFile` | resident file-backed memory는 얼마인가? | shared library와 mmap file 등이 포함될 수 있다. |
| `smaps_rollup` | mapping별 정보를 합한 더 상세한 값은 무엇인가? | 더 정확하지만 읽는 비용이 더 크다. |

`VmRSS`는 `RssAnon + RssFile + RssShmem`으로 설명된다. 그러나 application object의 합을 직접 보여주는 값은 아니다.

## RSS가 내려가지 않는 경우

```text
object drop됨
  ├─ collection capacity가 아직 owner에게 남음
  ├─ allocator가 free block을 retention
  ├─ page 일부에 다른 live allocation이 존재
  ├─ file/shared page가 resident
  └─ kernel accounting 관측 시점/정확도 차이
```

따라서 RSS graph만으로 leak을 확정하지 않는다. Object count, collection capacity, allocator active/retained metrics, `smaps_rollup`, cgroup `memory.current`를 함께 본다.

## 보장 경계

### 이 장이 보장하는 설명

- virtual address와 resident physical page가 다른 개념이라는 점
- RSS가 Rust heap live bytes의 정확한 합이 아니라는 점

### 이 장이 보장하지 않는 것

- `/proc`의 한 숫자만으로 원인을 확정할 수 있다는 주장
- RSS 감소가 application의 deallocation과 항상 같은 시점에 발생한다는 주장
- file-backed resident page가 모두 회수 불가능한 memory라는 주장

### 출처와 권위

- **OS 공식:** [Linux `/proc` filesystem](https://docs.kernel.org/filesystems/proc.html), [`proc_pid_status(5)`](https://man7.org/linux/man-pages/man5/proc_pid_status.5.html), [`proc_pid_smaps(5)`](https://man7.org/linux/man-pages/man5/proc_pid_smaps.5.html)
- **보조 학습:** [High Assurance Rust — Memory Safety](https://highassurance.rs/chp4/_index.html)
