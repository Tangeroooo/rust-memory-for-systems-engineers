# Capacity, reserve, shrink

Collection memory를 제어할 때 `len`만 보면 부족하다. `capacity`는 이미 확보해 둔 저장 공간이고, 미래의 growth allocation과 현재 retained memory를 연결하는 핵심 값이다.

## `reserve`와 `reserve_exact`

- `reserve(additional)`: `len + additional`을 담을 수 있도록 capacity를 확보한다. 미래 growth를 고려해 더 많이 확보할 수 있다.
- `reserve_exact(additional)`: 최소 필요량에 가깝게 요청하지만 allocator가 더 큰 block을 줄 수 있다.
- 두 API는 일반적으로 allocation failure를 정상적인 `Result`로 돌려주지 않는 infallible 경로다.

```rust
fn main() {
    let mut values = Vec::<u64>::new();
    values.reserve(128);
    assert!(values.capacity() >= values.len() + 128);
}
```

## `try_reserve`

`try_reserve`와 `try_reserve_exact`는 capacity overflow 또는 allocator failure를 `TryReserveError`로 반환할 수 있다.

```rust
use std::collections::TryReserveError;

fn build(count: usize) -> Result<Vec<u64>, TryReserveError> {
    let mut values = Vec::new();
    values.try_reserve(count)?;
    for value in 0..count as u64 {
        values.push(value);
    }
    Ok(values)
}

fn main() {
    assert_eq!(build(4).unwrap(), vec![0, 1, 2, 3]);
}
```

성공 후 `capacity` 안의 개별 `push`는 재allocation하지 않는다는 `Vec`의 public guarantee를 활용할 수 있다. 그러나 loop body에서 호출하는 다른 함수, element의 `clone`, logging, formatting, dependency는 별도 allocation을 수행할 수 있다.

## `clear`, `truncate`, `shrink_to_fit`

| 동작 | element | capacity |
|---|---|---|
| `clear()` | 모두 drop | 유지 |
| `truncate(n)` | 뒤쪽 element drop | 유지 |
| `shrink_to(min)` | 유지 | 줄이기를 시도하되 최소 `max(len, min)` |
| `shrink_to_fit()` | 유지 | `len`에 가깝게 줄이기를 시도 |
| `drop(vec)` | 모두 drop | owned allocation을 deallocation 대상으로 전달 |

`shrink_to_fit`의 이름을 “RSS를 즉시 줄인다”로 읽으면 안 된다. Collection이 allocator에 더 작은 layout을 요청하는 단계와 allocator/OS가 page를 반환하는 단계가 남아 있다.

## Capacity policy

- 짧은 간격으로 비우고 다시 채운다면 capacity 재사용이 allocation churn을 줄인다.
- burst 이후 오랫동안 작은 크기로 유지된다면 explicit shrink나 buffer 교체를 검토한다.
- untrusted input이 capacity를 영구히 부풀릴 수 있다면 per-request buffer 재사용 정책에 상한을 둔다.

이는 workload 기반 policy다. “항상 shrink”와 “절대 shrink하지 않음”은 모두 일반 해답이 아니다.

## 보장 경계

### 이 장이 보장하는 설명

- `Vec`의 reported capacity 안에서 `push`가 재allocation하지 않는다는 public contract
- `clear`가 capacity를 유지하고 shrink API가 축소를 시도한다는 점

### 이 장이 보장하지 않는 것

- reserve growth factor, allocator의 실제 block size, RSS 감소량
- `try_reserve` 성공 이후 호출 graph 전체가 allocation-free라는 주장
- `reserve_exact`가 allocator에게 정확히 그 byte만 소비하게 한다는 주장

### 출처와 권위

- **구현 확인/public contract:** [`Vec`](https://doc.rust-lang.org/std/vec/struct.Vec.html), [`Vec::try_reserve`](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.try_reserve), [`String::try_reserve`](https://doc.rust-lang.org/std/string/struct.String.html#method.try_reserve), [`HashMap::try_reserve`](https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.try_reserve)
- **설계 배경:** [RFC 2116 — Alloc Me Maybe](https://github.com/rust-lang/rfcs/blob/master/text/2116-alloc-me-maybe.md)
