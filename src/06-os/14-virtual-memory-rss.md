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

## Anonymous memory는 “출처 불명”이 아니다

Linux에서 anonymous memory는 filesystem의 file로 backing되지 않은 memory를 뜻한다. Kernel 문서는 program의 heap과 stack에 대해 implicit anonymous mapping이 만들어지고, `mmap(MAP_ANONYMOUS)`로도 explicit mapping을 만들 수 있다고 설명한다. `MAP_PRIVATE` file mapping을 수정해 생긴 Copy-on-Write page도 anonymous page가 될 수 있다.

| 발생원 | allocator requested-byte counter에서 보이는가? | OS/cgroup에서 보이는 방식 |
|---|:---:|---|
| Rust/C heap allocation | 보통 예 | page가 resident하면 `RssAnon`/cgroup `anon`에 반영 가능 |
| allocator arena, size-class slack, metadata | user requested byte에는 정확히 안 보일 수 있음 | anonymous mapping/page로 반영 가능 |
| thread stack | 아니요 | stack의 resident anonymous page |
| 직접 `mmap(MAP_ANONYMOUS)` | global allocator를 우회할 수 있음 | anonymous mapping/page |
| `MAP_PRIVATE` file의 Copy-on-Write | 아니요 | 수정된 private anonymous page |
| native/FFI library의 별도 allocator | 경우에 따라 다름 | process/cgroup memory에는 포함 가능 |
| page cache, shared mapping | Rust heap counter 밖 | `RssFile`, `RssShmem`, cgroup `file` 등 |

따라서 `RssAnon` 증가는 곧 “Rust object leak”을 뜻하지 않는다. 반대로 allocator live byte가 안정적이어도 thread 증가, direct mapping, allocator retention 때문에 anonymous resident memory가 증가할 수 있다.

Allocator counter는 자신을 통과한 요청을 제한하는 데 유용하다. 그러나 requested byte, allocator backing, resident page, cgroup charge는 서로 다른 accounting boundary다.

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
- heap, stack, anonymous `mmap`, private Copy-on-Write page가 anonymous memory를 만들 수 있다는 점

### 이 장이 보장하지 않는 것

- `/proc`의 한 숫자만으로 원인을 확정할 수 있다는 주장
- RSS 감소가 application의 deallocation과 항상 같은 시점에 발생한다는 주장
- file-backed resident page가 모두 회수 불가능한 memory라는 주장

### 출처와 권위

- **OS 공식:** [Linux `/proc` filesystem](https://docs.kernel.org/filesystems/proc.html), [`proc_pid_status(5)`](https://man7.org/linux/man-pages/man5/proc_pid_status.5.html), [`proc_pid_smaps(5)`](https://man7.org/linux/man-pages/man5/proc_pid_smaps.5.html)
- **OS 공식:** [Linux memory management concepts — Anonymous Memory](https://docs.kernel.org/admin-guide/mm/concepts.html#anonymous-memory), [cgroup v2 `memory.stat`](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)
- **구현 확인:** [`GlobalAlloc` safety](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html#safety)
- **보조 학습:** [High Assurance Rust — Memory Safety](https://highassurance.rs/chp4/_index.html)
