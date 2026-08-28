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

## 실전 예제 — Allocator 밖에서 생기는 anonymous memory

저장소의 Linux 실험은 다음 세 경로를 같은 process에서 비교한다.

```text
Vec heap touch
  → Rust global allocator counter 증가
  → write된 page가 RssAnon에 반영될 수 있음

직접 mmap(MAP_ANONYMOUS) + page touch
  → Rust global allocator counter를 우회
  → RssAnon은 증가할 수 있음

새 thread의 stack page touch
  → stack mapping은 task의 Vec/Box accounting 밖
  → RssAnon은 증가할 수 있음
```

### 1. Global allocator가 보는 requested byte

```rust,ignore
struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE_REQUESTED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_REQUESTED_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
}
```

이 counter는 자신을 통과한 requested byte를 본다. Size-class rounding, allocator metadata, retention과 allocator를 우회한 mapping까지 측정하는 것이 아니다. Allocator 안에서 log나 heap allocation을 수행하면 re-entrance가 생길 수 있으므로 예제는 atomic counter만 사용한다.

### 2. Global allocator를 우회하는 anonymous `mmap`

```rust,ignore
let pointer = unsafe {
    mmap(
        std::ptr::null_mut(),
        8 * 1024 * 1024,
        PROT_READ | PROT_WRITE,
        MAP_PRIVATE | MAP_ANONYMOUS,
        -1,
        0,
    )
};

// Mapping 생성만으로 모든 page가 resident한다고 가정하지 않는다.
// Page마다 write해야 physical page와 RSS 변화를 관찰하기 쉽다.
for offset in (0..length).step_by(page_size) {
    unsafe { pointer.cast::<u8>().add(offset).write_volatile(0x5a) };
}
```

이 호출은 Rust `GlobalAlloc` method가 아니므로 global allocator의 requested-byte counter에 들어가지 않는다. 그러나 Linux process와 cgroup의 anonymous memory에는 포함될 수 있다.

### 3. Thread stack

```rust,ignore
let worker = std::thread::Builder::new()
    .stack_size(4 * 1024 * 1024)
    .spawn(|| {
        // Recursive frame의 stack page를 실제로 touch하는 실험을 수행한다.
        touch_stack_pages();
    })?;
worker.join().unwrap();
```

`stack_size`는 stack mapping의 크기와 관련된 요청이지 곧바로 같은 크기의 RSS를 뜻하지 않는다. 실제로 접근한 page가 resident anonymous memory로 관측될 수 있다. Thread runtime이 만드는 작은 heap allocation이 allocator counter에 보일 수도 있으므로, 실험에서는 개별 숫자의 완전한 일치가 아니라 변화 방향을 본다.

### 실행하기

Linux 또는 Linux container에서 다음을 실행한다.

```bash
cargo run -p memory-lab --bin linux_anonymous
```

전체 코드는 [`linux_anonymous.rs`](https://github.com/Tangeroooo/rust-memory-for-systems-engineers/blob/main/examples/memory-lab/src/bin/linux_anonymous.rs)에 있다. 예제는 `/proc/self/status`의 `VmRSS`, `RssAnon`과 global allocator requested byte를 함께 출력한다.

결과는 allocator, kernel, optimization, page size에 따라 달라진다. 다음과 같은 exact assertion을 작성하면 안 된다.

```text
direct mmap 8 MiB → RssAnon이 즉시 정확히 8192 KiB 증가한다
```

Page rounding, 이미 존재하는 mapping, sampling timing, allocator/runtime activity가 있기 때문이다. 이 실험의 목적은 **allocator counter에 없는 anonymous resident memory가 실제로 생길 수 있음**을 재현하는 것이다.

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
- Global allocator counter를 우회한 direct mapping도 process/cgroup anonymous memory에 포함될 수 있다는 점

### 이 장이 보장하지 않는 것

- `/proc`의 한 숫자만으로 원인을 확정할 수 있다는 주장
- RSS 감소가 application의 deallocation과 항상 같은 시점에 발생한다는 주장
- file-backed resident page가 모두 회수 불가능한 memory라는 주장
- 실험에서 요청한 byte와 `RssAnon` 변화가 정확히 일치한다는 주장

### 출처와 권위

- **OS 공식:** [Linux `/proc` filesystem](https://docs.kernel.org/filesystems/proc.html), [`proc_pid_status(5)`](https://man7.org/linux/man-pages/man5/proc_pid_status.5.html), [`proc_pid_smaps(5)`](https://man7.org/linux/man-pages/man5/proc_pid_smaps.5.html)
- **OS 공식:** [Linux memory management concepts — Anonymous Memory](https://docs.kernel.org/admin-guide/mm/concepts.html#anonymous-memory), [cgroup v2 `memory.stat`](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)
- **구현 확인:** [`GlobalAlloc` safety](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html#safety)
- **보조 학습:** [High Assurance Rust — Memory Safety](https://highassurance.rs/chp4/_index.html)
