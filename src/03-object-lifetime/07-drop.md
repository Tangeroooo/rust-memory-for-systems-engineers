# Drop, RAII, destructor scope

초기화된 값이 scope를 벗어나면 destructor가 실행된다. Type `T`가 `Drop`을 구현했다면 먼저 `T::drop`이 호출되고, 이어서 field의 destructor가 재귀적으로 실행된다. 이 규칙은 resource 정리의 출발점이지만 process 종료의 모든 경우에 실행을 보장하지는 않는다.

## `Drop`이 호출되는 흐름

```rust
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

struct Tracked(Arc<AtomicUsize>);

impl Drop for Tracked {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn main() {
    let drops = Arc::new(AtomicUsize::new(0));
    {
        let _value = Tracked(Arc::clone(&drops));
    }
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
```

`std::mem::drop(value)`는 특별한 destructor 호출 문법이 아니다. 값을 by-value로 받아 함수 끝에서 drop되도록 만드는 `fn drop<T>(_: T) {}` 형태의 함수다. `Drop::drop`을 직접 호출하는 것은 허용되지 않는다.

## Drop과 deallocation을 구분한다

```text
owner scope 종료
    ↓
T::drop
    ↓
field destructor
    ↓
소유 heap buffer의 deallocation 요청
    ↓
allocator가 block을 재사용하거나 OS에 반환
```

모든 `Drop`이 heap deallocation을 뜻하지 않는다. file descriptor, lock guard, socket, tracing span도 `Drop`으로 정리될 수 있다. 반대로 `Vec::clear`는 element는 drop하지만 buffer capacity를 유지한다.

## Destructor가 실행되지 않을 수 있는 경로

- `std::mem::forget`으로 값을 의도적으로 잊은 경우
- `Rc`/`Arc` strong cycle이 남은 경우
- `std::process::abort`, `std::process::exit` 등 unwinding 없는 process 종료
- `panic = "abort"`에서 panic으로 종료
- OOM killer나 cgroup OOM kill처럼 kernel이 process를 종료

따라서 durability나 외부 protocol correctness를 destructor 단독에 의존하지 않는다. 명시적 `flush`, `commit`, `close`와 crash recovery가 필요한 영역이 있다.

## 보장 경계

### 이 장이 보장하는 설명

- 정상적인 scope exit와 unwinding에서 destructor scope가 작동하는 방식
- `Drop`과 field destruction의 기본 순서

### 이 장이 보장하지 않는 것

- 모든 process 종료 경로에서 destructor가 실행된다는 주장
- `Drop` 실행 직후 RSS가 감소한다는 주장
- destructor만으로 durable write가 보장된다는 주장

### 출처와 권위

- **언어 규범:** [Rust Reference — Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **공식 학습서:** [Running Code on Cleanup with the `Drop` Trait](https://doc.rust-lang.org/book/ch15-03-drop.html)
- **구현 확인:** [`std::mem::drop`](https://doc.rust-lang.org/std/mem/fn.drop.html), [`std::mem::forget`](https://doc.rust-lang.org/std/mem/fn.forget.html)
