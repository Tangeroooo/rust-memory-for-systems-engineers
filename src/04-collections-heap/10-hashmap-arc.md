# HashMap과 Arc의 비용

`HashMap<K, V>`와 `Arc<T>`는 모두 ownership을 안전하게 표현하지만, memory footprint를 무료로 만들지는 않는다. Metadata, spare capacity, hashing state, reference count, alignment가 추가된다.

## `HashMap<K, V>`

`HashMap`의 `len`은 entry 수이고 `capacity`는 재allocation 없이 담을 수 있는 최소 entry 수를 나타낸다. 내부 table layout, bucket metadata, growth strategy는 public API의 안정된 계약으로 간주하면 안 된다.

```rust
use std::collections::HashMap;

fn main() {
    let mut table = HashMap::with_capacity(100);
    table.insert("rss", 1_u64);

    assert_eq!(table.len(), 1);
    assert!(table.capacity() >= table.len());
}
```

Memory estimate에는 최소한 다음이 들어간다.

```text
table storage
+ key가 소유한 nested allocation
+ value가 소유한 nested allocation
+ spare capacity / bucket metadata
+ allocator rounding과 fragmentation
```

## `Arc<T>`

`Arc<T>`는 atomic reference counting을 사용해 shared ownership을 제공한다. `Arc::clone`은 일반적으로 내부 `T`를 clone하지 않고 strong count를 증가시킨다. 그 대신 clone/drop 경로에 atomic operation 비용이 생긴다.

```rust
use std::sync::Arc;

fn main() {
    let data = Arc::new(vec![1_u8, 2, 3]);
    let shared = Arc::clone(&data);

    assert!(Arc::ptr_eq(&data, &shared));
    assert_eq!(Arc::strong_count(&data), 2);
}
```

다음 셋을 구분한다.

- `Arc::clone(&arc)`: 같은 allocation의 shared ownership 추가
- `(*arc).clone()`: 내부 `T`를 clone할 수 있으며 nested allocation 발생 가능
- `Arc::make_mut(&mut arc)`: 다른 strong owner가 있으면 clone-on-write가 발생할 수 있음

## Ownership graph를 비용 graph로 읽기

복잡한 server state에서는 type graph를 다음 두 관점으로 본다.

1. **drop graph:** 어떤 strong owner가 사라져야 object가 파괴되는가?
2. **allocation graph:** object 하나가 몇 개의 nested allocation을 소유하는가?

`Arc<HashMap<String, Vec<u8>>>`는 한 줄의 type이지만 `Arc` control allocation, hash table storage, 각 `String`, 각 `Vec<u8>`의 buffer를 포함할 수 있다.

## 보장 경계

### 이 장이 보장하는 설명

- `HashMap::len`과 `capacity`의 public 의미
- `Arc`가 shared ownership을 제공한다는 점과 `Arc::clone`의 의미

### 이 장이 보장하지 않는 것

- `HashMap` bucket layout이나 load factor가 영구히 고정된다는 주장
- `Arc`가 내부 값의 mutation을 자동 동기화한다는 주장
- `size_of::<T>()`만으로 nested allocation을 포함한 footprint를 얻는다는 주장

### 출처와 권위

- **구현 확인/public contract:** [`HashMap`](https://doc.rust-lang.org/std/collections/struct.HashMap.html), [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html)
- **공식 학습서:** [`Rc<T>`, the Reference-Counted Smart Pointer](https://doc.rust-lang.org/book/ch15-04-rc.html)
- **보조 학습:** [Rust Performance Book — Heap Allocations](https://nnethercote.github.io/perf-book/heap-allocations.html)
