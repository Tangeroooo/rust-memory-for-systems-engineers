# 실습: estimate에서 deterministic 반환까지

이 장의 목표는 “적당한 숫자를 reserve하고 나중에 돌려준다”를 넘어서는 것이다. **고정 폭 record를 정렬하는 operator 하나**를 구현하면서, 무엇을 계산하고 언제 승인하며 누가 반환하는지를 따라간다.

핵심은 다음 세 규칙이다.

1. 최초 estimate와 실행 중 allocation 크기를 구분한다. 다음 allocation의 Layout은 요청하기 전에 계산할 수 있다.
2. 재할당 중 old와 new가 함께 살아 있다면 **두 allocation 모두** charge한다.
3. Grant는 task scope가 아니라 **storage ownership**에 붙이고, deallocation 뒤에 반환한다.

## 1. 먼저 budget의 대상을 고정한다

이번 operator는 `Row { key: u64, value: u64 }`를 입력받아 key 순으로 정렬한다. Row에는 `String`, `Vec`, pointer로 이어지는 별도 heap allocation이 없다. Wire format도 두 개의 little-endian `u64`, 즉 16 byte다.

| 대상 | 이 예제의 처리 |
|---|---|
| Row array의 storage | `Layout::array::<Row>(capacity).size()`만큼 charge |
| 초기화되지 않은 spare capacity | **포함**. `len`이 아니라 allocation 전체를 charge |
| Growth 중 old + new storage | **둘 다 포함** |
| 정렬 scratch heap | `sort_unstable_by_key`와 allocation 없는 `u64` key를 사용하여 없음 |
| 입력 buffer, pool metadata, stack, logging, 출력 전송 buffer | 별도 domain. 이번 pool에 포함되지 않음 |
| allocator size class/metadata/retention, physical page | 별도 consumption 관측 대상. Requested byte와 같지 않음 |

`sort_unstable_by_key`의 in-place/no-allocation 성질은 [standard library 계약](https://doc.rust-lang.org/std/primitive.slice.html#method.sort_unstable_by_key)이다. Key를 만드는 closure가 자체적으로 allocation하면 그 비용은 별개다. 이 예제는 `u64` key만 반환한다.

**“Requested byte에 대한 정확한 상한”과 “allocator가 확보한 모든 메모리의 상한”은 다르다.** 이번 장은 전자를 실제 allocation 경로에 연결한다. [Anonymous memory 실험](../06-os/14-virtual-memory-rss.md)은 후자의 관측값이 왜 달라지는지 보여준다.

**이 실습은 전체 애플리케이션이 fallible하다고 가정하지 않는다.** 정확히 통제하는 대상은 위 표의 row storage뿐이다. 앞단에서 `serde_json`으로 별도 `Vec<String>`을 만들거나 뒤에서 결과를 `String`으로 변환하면, 그 allocation은 이 pool의 reserve 숫자에 자동 포함되지 않는다. 전체 시스템에서는 [외부 crate가 섞인 경로](../01-mental-model/00-why-admission.md#실제-library를-넣으면-드러나는-차이)를 별도로 분류하고 headroom·입력 제한·격리를 적용해야 한다.

## 2. Estimate를 어떻게 계산하는가

입력의 예상 wire 길이가 64 byte라면 다음과 같다.

```text
estimated_rows = estimated_wire_bytes / WIRE_ROW_BYTES
               = 64 / 16 = 4 rows

initial_bytes  = Layout::array::<Row>(4).size()
               = 4 × 16 = 64 B
```

`size_of::<Row>()`가 16인지 compile-time assertion으로 확인한다. 다른 type이라면 padding까지 포함한 `Layout`으로 계산해야 한다. 나눗셈 전에 record 단위 길이를 검사하며, `Layout::array`는 overflow와 `isize::MAX` 범위 초과를 검사한다.

```rust,ignore
{{#include ../../examples/memory-lab/src/tracked_sort.rs:plan}}
```

여기서 `SortPlan`을 만드는 것만으로는 reserve도 allocate도 하지 않는다. `initial_bytes`는 plan의 계산 결과다. 실제 constructor는 row 수로부터 Layout을 다시 계산하여 승인받는다.

### 정확한 정보와 estimate를 구분한다

- 변경되지 않는 고정 폭 입력 전체의 byte 길이를 알고, record당 output이 하나라면 row 수는 정확하다. 이 경우 처음부터 전체 capacity를 확보하여 growth를 없앨 수 있다.
- Stream header나 과거 통계가 “4개 정도”라고 알려준다면 이는 estimate다. 실제 다섯 번째 record가 오면 아래의 incremental 경로를 실행한다.
- 압축된 입력 크기만으로 decompressed row 수를 확정할 수 없다. 압축 해제 출력 자체에 별도의 byte limit과 fallible growth 경계가 필요하다.

**숫자를 미리 전부 맞히는 것이 correctness 조건이 아니다.** 초기 예측이 틀려도 다음 storage를 요청하기 직전에 그 크기를 계산하고 승인받을 수 있어야 한다.

### `reserve`라는 이름의 단위를 혼동하지 않는다

| 호출 | 숫자의 의미 | 실제 memory allocation |
|---|---|---|
| `MemoryPool::try_reserve(64)` | application 권한 64 **byte** | 이 pool에서는 counter와 RAII token만 변경 |
| `Vec::<Row>::try_reserve(4)` | 현재 `len`에서 추가할 4 **element** | 필요할 때 수행. 최소 `len + 4` capacity |
| 이 예제의 `Block::try_new(..., 4)` | 총 capacity 4 **row** | Layout 64 byte를 계산하여 승인 후 요청 |

`Vec::try_reserve`의 숫자는 byte도, 기존 capacity에 더할 숫자도 아니다. `try_reserve_exact`도 allocator usable size나 RSS가 정확히 그 값이라는 보장은 없다. 이 때문에 이번 실습은 `Vec`의 성장 정책에 기대지 않고 작은 전용 storage wrapper를 사용한다.

## 3. Admission: grant를 먼저 얻는다

```rust,ignore
{{#include ../../examples/memory-lab/src/tracked_sort.rs:allocate}}
```

순서가 중요하다.

1. Layout을 계산한다. 불가능한 크기이면 allocation 전에 error다.
2. `pool.try_reserve(layout.size())`로 grant를 얻는다. 거절되면 allocator는 호출되지 않는다.
3. 선택한 allocator를 호출한다. 이 실습은 `System`을 명시적으로 전달한다.
4. Null이면 `Err`로 돌아온다. 아직 지역 변수인 reservation은 `Drop`되어 새 grant가 반환된다.
5. 성공하면 pointer와 Layout, allocator reference, reservation을 하나의 `Block`이 소유한다.

`System`을 직접 호출하는 경로는 프로그램이 선택한 `#[global_allocator]`와 독립적일 수 있다. 따라서 이 block은 **이 pool에서 추적되지만 별도의 global allocator hook에서는 보이지 않을 수 있다.** 여기에서도 “어느 경로의 counter인가”를 밝혀야 한다. [System 공식 문서](https://doc.rust-lang.org/std/alloc/struct.System.html)

이 wrapper는 null을 처리하는 fallible 경로를 제공한다. Allocator 자체가 abort하거나 OS가 process를 kill하면 `Err`를 얻을 수 없다. Physical memory를 확보해 두었다는 보장도 아니다.

## 4. Growth: 차액만 reserve하면 왜 부족한가

초기 capacity가 4이고 다섯 번째 row를 넣어야 한다고 하자. 이 예제는 capacity를 두 배로 늘린다.

```text
old capacity = 4 rows →  64 B
new capacity = 8 rows → 128 B

steady-state 차액 = 128 - 64 = 64 B
copy 중 peak      = 128 + 64 = 192 B
추가 grant        = new 전체 128 B
```

기존 64 byte가 살아 있는 동안 새로운 128 byte를 별도로 allocate하기 때문이다. `realloc`이 운 좋게 in-place로 성공한다고 가정하지 않는다.

```rust,ignore
{{#include ../../examples/memory-lab/src/tracked_sort.rs:grow}}
```

부족할 때마다 `Reservation::try_grow(new_bytes)`로 같은 token을 늘리는 설계도 가능하다. 이 예제는 **allocation마다 별도의 token**을 붙인다. 두 token이 공존하는 동안 pool의 reservation 합이 증가하고, old block을 버릴 때 old token만 자동 반환된다. 둘은 같은 invariant를 표현하는 서로 다른 ownership 설계다.

### 숫자로 따라가는 한 번의 실행

다른 작업이 없는 pool, limit `192 B`, initial estimate 4 rows를 가정한다.

| 사건 | 살아 있는 row allocation | Pool reservation | 반환한 grant |
|---|---:|---:|---:|
| Plan만 생성 | 0 | 0 | 0 |
| 초기 grant 승인, alloc 직전 | 0 | 64 B | 0 |
| 첫 allocation 성공 | 64 B | 64 B | 0 |
| 4개 row를 채움 | 64 B | 64 B | 0 |
| 다섯 번째 row: new grant 승인 | 64 B | **192 B** | 0 |
| New alloc 성공, copy 중 | **64 + 128 B** | **192 B** | 0 |
| Old dealloc 완료 | 128 B | 192 B → **128 B** | **64 B** |
| 정렬 완료, output으로 move | 128 B | 128 B | 0 |
| 최종 output Drop: dealloc 후 release | 0 | **0** | **128 B** |

Limit이 `160 B`이면 steady-state `128 B`는 들어가지만 growth peak `192 B`는 들어가지 않는다. 따라서 **new allocator 호출 전에 거절**한다. Caller는 old 데이터를 가지고 spill하거나 작업을 실패시킬 수 있다. 단순히 차액 64 byte만 확인하면 이 peak를 놓친다.

## 5. 실패하면 무엇이 남는가

| 실패 지점 | 함수가 반환한 뒤의 state | Caller의 선택 |
|---|---|---|
| Initial grant 거절 | allocation 없음, grant 없음 | bounded queue / reject |
| Initial alloc이 null 반환 | 새 grant 반환, allocation 없음 | task error |
| Growth grant 거절 | old 4 rows와 64 B grant 유지 | spill / reject / 재시도 정책 |
| Growth alloc이 null 반환 | **새 128 B grant만 rollback**, old 유지 | 같은 buffer로 재시도 가능 |
| 상위 task가 `?`로 error 반환 | old buffer도 Drop → dealloc → release | task 종료 |

동기 함수 내부의 grant 획득과 block ownership 설정 사이에는 `await`가 없다. 오류 경로에는 RAII owner가 남아 있다. 다만 이런 구조를 async 함수로 바꾼다면 **모든 suspension point에서 pending allocation과 grant를 누가 소유하는지** 다시 확인해야 한다.

여러 task가 old storage를 가진 채 추가 grant를 영원히 기다리면 서로 진행하지 못할 수 있다. 이 예제는 기다리지 않고 거절한다. 운영에서는 spill용 별도 reserve, bounded concurrency, 재시도 제한 등을 정책으로 추가한다.

## 6. 언제 반환하는가: `clear`, task 완료, output 소멸

“작업 종료 시 reservation을 반환한다”는 문장은 결과 storage도 그때 소멸하는 경우에만 맞다.

| 연산 | Storage | Reservation |
|---|---|---|
| `buffer.clear()` | capacity 유지 | **유지** |
| `buffer.release_storage()` | dealloc | 그 뒤 반환 |
| `buffer.finish()` | 정렬 후 output으로 move | output과 함께 move, **반환하지 않음** |
| `drop(output)` | 최종 owner가 dealloc | 그 뒤 반환 |
| caller의 borrow가 끝남 | owner가 살아 있으면 유지 | 유지 |

다음 `Drop` body가 끝난 뒤 Rust가 struct field를 파괴하므로 `_reservation`이 마지막에 반환된다.

```rust,ignore
{{#include ../../examples/memory-lab/src/tracked_sort.rs:release}}
```

Pointer, Layout과 reservation을 따로 꺼내는 public API를 제공하지 않는다. `SortedRows`도 내부 buffer와 grant를 함께 소유한다. `Arc`로 결과를 공유하는 구조라면 **마지막 strong owner가 사라지는 시점**까지 같은 원칙을 유지해야 하며, reference cycle과 leak은 별도로 막아야 한다.

### Task-level cleanup

```rust,ignore
{{#include ../../examples/memory-lab/src/tracked_sort.rs:task}}
```

Cancellation flag를 세웠다는 사실만으로 즉시 반환되지는 않는다. Task가 cancellation을 관측하고 owner를 파괴하는 지점에서 반환된다. 외부가 worker의 종료를 기다리지 않은 채 grant부터 돌려주면 아직 살아 있는 storage를 중복 승인할 수 있다.

## 7. 여기서 deterministic하다는 뜻

이 예제의 deterministic 성질은 **ownership 사건과 accounting 순서**에 대한 것이다.

```text
모든 row allocation 직전: 대응 grant가 이미 존재한다.
모든 grant 반환 직전: 대응 storage가 이미 deallocate되었다.
모든 시점: live requested row bytes <= reserved row bytes <= pool limit
```

같은 입력과 단일 task, 같은 failure injection이면 byte 변화와 반환 사건을 동일하게 test할 수 있다. GC 시점이나 RSS 감소를 기다리지 않는다. 그러나 다음을 뜻하지는 않는다.

- 동시 요청 중 누가 먼저 승인될지, cancellation을 몇 ms 만에 관측할지의 결정성
- deallocation 후 allocator가 OS page를 언제 반환할지의 결정성
- `panic=abort`, OOM kill, process 강제 종료에서도 destructor가 실행된다는 보장
- `mem::forget`, leak, cycle을 포함한 모든 safe Rust 프로그램에서 반환이 보장된다는 주장

Ordinary panic의 **unwind 구성**에서는 owner가 파괴되는지 test할 수 있다. 이는 OOM abort를 `catch_unwind`로 복구한다는 뜻이 아니다.

## 8. 실행하고 failure를 재현한다

이 장의 코드 block은 설명용 복사본이 아니라 실제 `.rs`의 anchor를 mdBook에서 include한 것이다. 외부 crate를 사용하는 조각이므로 `mdbook test`에서는 `ignore`하며, **전체 구현은 `cargo test`와 실행 예제에서 검증한다.**

```bash
cargo run -p memory-lab --bin budget_timeline
cargo test -p memory-lab tracked_sort
```

대표 실행:

```text
plan: rows=4, initial=64 B, reserved=0 B
admitted: reserved=64 B
4 rows: capacity=4, reserved=64 B
5 rows: capacity=8, reserved=128 B (growth peak: 192 B)
task finished: rows=5, reserved=128 B
output dropped: reserved=0 B
```

실행 프로그램의 peak는 정책 계산값이며, test의 `Probe` allocator가 **alloc 호출 순간 reservation이 192 B인지** 별도로 확인한다. Test는 실제 OOM을 만들지 않는다. 지정한 allocation 한 번만 null로 실패시키고 아래의 사건을 검사한다.

| Test | 검증하는 규칙 |
|---|---|
| `plan_uses_wire_count_and_checked_layout` | 입력 단위, byte 계산, 잘못된 길이와 overflow |
| `growth_charges_overlap_and_output_keeps_grant` | alloc 직전 64/192 B, dealloc 중 grant 유지, output Drop 후 0 |
| `steady_size_fits_but_overlap_does_not` | limit 160 B에서는 new alloc 미호출, old data 유지 |
| `failed_allocation_rolls_back_only_new_grant_then_retry_succeeds` | null 이후 64 B 복원, 같은 buffer로 재시도 |
| `initial_denial_and_initial_null_leave_no_grant` | 초기 실패 후 grant leak 없음 |
| `clear_keeps_storage_release_storage_returns_it` | logical clear와 storage 반환의 차이 |
| `empty_plan_never_calls_allocator_with_zero_layout` | 크기 0은 allocation하지 않음 |
| `task_error_and_cooperative_cancellation_release_storage` | 상위 error와 cancellation에서 dealloc/release |
| `repeated_growth_preserves_every_row_and_matches_reference_sort` | 여러 초기 capacity에서 1,025 rows를 반복 growth해 reference 정렬과 비교 |
| `unwind_drops_storage_but_is_not_an_oom_recovery_mechanism` | 일반 panic unwind의 cleanup |

[전체 구현](https://github.com/Tangeroooo/rust-memory-for-systems-engineers/blob/main/examples/memory-lab/src/tracked_sort.rs) · [실행 프로그램](https://github.com/Tangeroooo/rust-memory-for-systems-engineers/blob/main/examples/memory-lab/src/bin/budget_timeline.rs)

## 9. 실제 DB로 옮길 때의 선택

| 선택 | 장점 | 비용 / 위험 |
|---|---|---|
| 전체 row 수를 알면 한 번에 reserve | 성장 peak 없음, 계산 단순 | 길이를 모르는 stream에는 직접 적용 불가 |
| 이 예제처럼 contiguous grow | 연속 storage, 빠른 순회 | old + new peak, copy 비용 |
| Bounded page/chunk 단위 growth | 기존 page를 복사하지 않고 새 page만 charge | page index·정렬 scratch도 charge 필요, 연속 slice API 사용 어려움 |

현재 교재에서는 **contiguous growth를 먼저 이해한 뒤 page/chunk 설계로 확장**하는 순서를 권한다. Allocation 경로가 드러나므로 잘못된 차액 계산을 test로 잡기 쉽다.

가변 길이 key의 hash aggregation에서는 `estimated_distinct_keys × size_of::<Entry>()`만으로 부족하다. Hash table bucket/control storage, key payload, collision/growth 정책, old/new overlap을 구분해야 한다. 표준 `HashMap` 내부 allocation 크기를 임의의 상수로 가정하지 말고, 실제 경로를 측정하거나 layout을 통제하는 table/arena를 선택한다. **초기 cardinality는 estimate여도, 새 bucket/page allocation의 byte 승인은 실제 요청 크기와 연결**되어야 한다.

적용 순서는 다음과 같다.

- **단기:** 큰 buffer 하나에 ownership+grant wrapper를 적용하고 실제 growth/실패/반환 순서를 test한다.
- **중기:** nested string, output queue, scratch, spill buffer까지 accounting domain을 넓히고 우회 경로를 점검한다.
- **장기:** hierarchical pool, fairness, pressure 기반 admission, bounded arena와 cgroup containment를 결합한다.

## 보장 경계

### 이 장이 보장하는 설명

- 제공된 예제에서 requested row storage의 크기를 Layout으로 계산하고 allocation 전에 승인하는 방법
- old/new overlap, 실패 rollback, output ownership, dealloc-before-release의 test 가능한 규칙
- estimate의 정확도와 enforcement의 정확도가 서로 다른 문제라는 점

### 이 장이 보장하지 않는 것

- Row 이외의 입력·metadata·native allocation이나 process RSS까지 이 pool에 포함된다는 주장
- 임의의 allocator가 항상 null로 실패하거나, allocation 성공 후 page fault가 반드시 성공한다는 주장
- Unsafe storage wrapper의 일반적인 production 검증 완료. 이 예제는 Copy Row에 한정한 학습 코드다.
- 범용 collection, nested destructor, async executor, thread 간 buffer 전송까지 구현했다는 주장. 현재 wrapper는 worker-local 예제다.

### 출처와 권위

- **Standard library 계약:** [`Layout::array`](https://doc.rust-lang.org/std/alloc/struct.Layout.html#method.array), [`GlobalAlloc::alloc / dealloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html), [`System`](https://doc.rust-lang.org/std/alloc/struct.System.html)
- **Standard library 계약:** [`Vec::try_reserve`](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.try_reserve), [`sort_unstable_by_key`](https://doc.rust-lang.org/std/primitive.slice.html#method.sort_unstable_by_key)
- **언어 규범:** [Rust Reference — Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **고급/unsafe 보조:** [Rustonomicon — Implementing Vec](https://doc.rust-lang.org/nomicon/vec/vec.html). 이 예제는 이를 대체하는 범용 Vec 구현이 아니다.
- **이 책의 설계·검증:** `SortPlan`, `Block`, `SortBuffer`, `SortedRows`와 test invariant는 표준 API의 보장이 아니라 이 저장소가 선택한 contract다.
