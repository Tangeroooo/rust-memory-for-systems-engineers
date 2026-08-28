# C/C++에서 Rust로 옮겨오는 mental model

C++ 경험은 좋은 출발점이지만 1:1 대응표는 아니다. 다음 대응은 **학습을 위한 유사성**이지 ABI, 구현, semantics의 동일성 선언이 아니다.

| C++의 익숙한 개념 | Rust에서 가까운 출발점 | 중요한 차이 |
|---|---|---|
| RAII / destructor | ownership + `Drop` | Rust는 move 이후 이전 binding의 사용을 정적으로 막는다. |
| `std::unique_ptr<T>` | `Box<T>` | `Box`도 ownership 규칙 전체에 참여하며 null이 아니다. |
| `std::vector<T>` | `Vec<T>` | `Vec`의 public guarantee는 pointer/capacity/length triplet이지만 field order와 growth factor는 미지정이다. |
| `std::string` | `String` | UTF-8 invariant를 유지하며 내부적으로 `Vec<u8>`에 가까운 표현을 사용한다. |
| `std::shared_ptr<T>` | `Rc<T>` / `Arc<T>` | `Rc`는 thread-safe하지 않다. `Arc`는 atomic reference count이지 내부 값의 mutation을 자동 동기화하지 않는다. |
| `std::weak_ptr<T>` | `Weak<T>` | strong cycle을 끊는 non-owning handle이다. |
| move constructor | Rust `move` | Rust의 move는 user-defined move constructor 호출 모델이 아니라 ownership의 의미론적 이전이다. |
| `const T&` / `T&` | `&T` / `&mut T` | `&mut T`는 단순한 “writable pointer”보다 강한 exclusive access 계약이다. |

## RAII에서 시작하되 여기서 멈추지 않는다

```cpp
{
    std::vector<int> values;
} // destructor
```

```rust
fn main() {
    {
        let values = Vec::<i32>::new();
        assert!(values.is_empty());
    } // Drop
}
```

두 코드 모두 scope exit에서 소유 resource를 정리하는 방향을 갖는다. 그러나 Rust 학습의 핵심은 destructor 자동 호출만이 아니다. compiler가 owner 이동, shared borrow, mutable borrow의 관계를 프로그램 전체 type checking에 포함한다.

## Rust move는 물리적 복사를 약속하지 않는다

```rust
fn main() {
    let first = String::from("memory");
    let second = first;
    assert_eq!(second, "memory");
    // println!("{first}"); // compile error: value used after move
}
```

중요한 사실은 `first`의 ownership이 `second`로 이동했다는 점이다. 실제 machine code가 몇 byte를 복사했는지는 최적화와 구현의 문제다. public semantics를 설명할 때 “pointer 세 개가 무조건 bitwise copy된다” 같은 구현 추정에 의존하지 않는다.

## `Arc<T>`는 `Arc<Mutex<T>>`가 아니다

```rust
use std::sync::{Arc, Mutex};

fn main() {
    let counter = Arc::new(Mutex::new(0_u64));
    let cloned = Arc::clone(&counter);

    *cloned.lock().unwrap() += 1;
    assert_eq!(*counter.lock().unwrap(), 1);
}
```

- `Arc`: allocation의 shared ownership과 atomic reference count
- `Mutex`: 내부 값에 대한 synchronized mutable access
- `T`: 실제 domain state

세 역할을 한 단어 “shared pointer”로 뭉치면 allocation lifetime과 concurrency control을 혼동한다.

## 보장 경계

### 이 장이 보장하는 설명

- C++ 개발자가 Rust type을 이해하기 위한 유사점과 차이
- Rust move가 ownership transfer라는 semantic event라는 설명

### 이 장이 보장하지 않는 것

- C++ type과 Rust type의 layout, ABI, 예외 안전성, thread semantics가 같다는 주장
- `Arc<T>`만 사용하면 내부 값에 대한 concurrent mutation이 안전해진다는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — Ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html), [Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- **구현 확인:** [`Box`](https://doc.rust-lang.org/std/boxed/struct.Box.html), [`Vec`](https://doc.rust-lang.org/std/vec/struct.Vec.html), [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html)
- **보조 학습:** [High Assurance Rust](https://highassurance.rs/)
