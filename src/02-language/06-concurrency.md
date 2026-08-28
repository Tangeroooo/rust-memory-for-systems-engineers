# Concurrency와 data race

Rust가 막는 것은 **data race**다. 프로그램이 원하는 순서와 다른 결과를 내는 넓은 의미의 race condition, deadlock, starvation, priority inversion까지 자동으로 해결하지는 않는다.

## `Send`와 `Sync`

- `Send`: 값을 다른 thread로 move해도 안전함을 나타낸다.
- `Sync`: `&T`를 여러 thread 사이에서 공유해도 안전함을 나타낸다.

대부분의 type은 구성 요소에 따라 이 auto trait들을 자동으로 구현한다. raw pointer를 감싼 custom abstraction이나 FFI resource가 수동으로 `unsafe impl Send/Sync`를 제공한다면 그 구현자가 invariant를 증명해야 한다.

## Ownership, mutability, synchronization은 서로 다르다

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let values = Arc::new(Mutex::new(Vec::<u64>::new()));
    let worker_values = Arc::clone(&values);

    thread::spawn(move || {
        worker_values.lock().unwrap().push(1);
    })
    .join()
    .unwrap();

    assert_eq!(&*values.lock().unwrap(), &[1]);
}
```

여기서 역할은 분리된다.

```text
Arc     → allocation의 shared ownership
Mutex   → mutable access의 synchronization
Vec     → element와 capacity의 storage
```

`Arc<Vec<T>>`는 여러 thread가 같은 `Vec`을 소유하게 하지만 `Vec::push`를 가능하게 하지 않는다. `Arc<Mutex<Vec<T>>>`는 mutation을 직렬화하지만 queue 크기의 상한을 정하지 않는다.

## Memory pressure의 concurrency 문제

동시에 실행되는 task가 각자 최대 1 GiB를 사용할 수 있다면 task 하나의 correctness만 검사해서는 부족하다.

```text
peak process memory
≈ fixed overhead
  + Σ(concurrent task reservations)
  + untracked allocations
  + allocator/OS overhead
```

따라서 concurrency limit과 memory admission을 함께 설계해야 한다. semaphore가 task 개수를 제한하더라도 task별 memory 편차가 크면 byte 단위 reservation이 별도로 필요하다.

## 보장 경계

### 이 장이 보장하는 설명

- sound한 safe Rust에서 data race를 방지하는 type-level 기반
- `Arc`, `Mutex`, collection이 각각 다른 책임을 가진다는 분리

### 이 장이 보장하지 않는 것

- deadlock, logical race, starvation 방지
- `Arc` 사용만으로 내부 mutation이 동기화된다는 주장
- thread/task 개수 제한이 memory upper bound와 같다는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — Extensible Concurrency with `Send` and `Sync`](https://doc.rust-lang.org/book/ch16-04-extensible-concurrency-sync-and-send.html)
- **구현 확인:** [`Send`](https://doc.rust-lang.org/std/marker/trait.Send.html), [`Sync`](https://doc.rust-lang.org/std/marker/trait.Sync.html), [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html)
- **고급/unsafe:** [Rustonomicon — Send and Sync](https://doc.rust-lang.org/nomicon/send-and-sync.html)
