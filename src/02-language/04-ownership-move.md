# Ownership과 move

Ownership은 각 값에 그 값을 책임지는 owner가 있도록 하고, owner가 scope를 벗어나면 정리 절차를 시작하게 하는 언어 규칙이다. 이것은 garbage collector 없이 memory safety를 제공하는 기반이지만, heap 사용량을 측정하거나 제한하는 runtime subsystem은 아니다.

## 세 가지 규칙

Rust Book의 학습 모델은 다음 세 규칙으로 요약된다.

1. 각 값에는 owner가 있다.
2. 한 시점에 owner는 하나다.
3. owner가 scope를 벗어나면 값이 drop된다.

`Copy` type은 대입 뒤 두 binding이 각각 독립된 값을 가진다. `String`, `Vec<T>`, `Box<T>`처럼 `Copy`가 아닌 type은 대입이나 by-value 호출에서 ownership이 move된다.

```rust
fn consume(value: String) -> usize {
    value.len()
}

fn main() {
    let message = String::from("ownership");
    let length = consume(message);
    assert_eq!(length, 9);
    // message는 move되었으므로 여기서 사용할 수 없다.
}
```

## Ownership은 storage location이 아니다

“소유한다”와 “heap에 있다”는 다른 분류다.

```rust
fn main() {
    let stack_value = 42_u64;          // 값 자체는 보통 stack frame에 놓인다.
    let heap_owner = Box::new(42_u64); // Box가 heap allocation을 소유한다.

    assert_eq!(stack_value, *heap_owner);
}
```

두 값 모두 owner가 있다. ownership 규칙만 보고 stack/heap 위치나 allocation 횟수를 단정할 수 없다. optimizer가 관측 불가능한 allocation을 제거할 수도 있으므로 allocator hook의 호출 횟수를 언어 semantics로 취급해서도 안 된다.

## Move와 clone을 구분한다

- `move`: 기존 값의 ownership을 새 place로 이전한다. 논리적으로 새 heap allocation이 필요하지 않다.
- `clone`: type이 정의한 방식으로 새 값을 만든다. heap-owning type은 추가 allocation을 일으킬 수 있다.
- `copy`: `Copy` type의 값 복제이며 source를 계속 사용할 수 있다.

성능 분석에서는 `move`보다 숨어 있는 `clone`, `collect`, formatting, growth allocation을 먼저 찾는 편이 유용하다.

## 보장 경계

### 이 장이 보장하는 설명

- move 이후 이전 binding을 safe Rust에서 사용할 수 없다는 점
- owner의 scope 종료가 destructor 실행과 연결된다는 점

### 이 장이 보장하지 않는 것

- 모든 move가 특정한 machine-level `memcpy`로 구현된다는 주장
- ownership이 allocation count, peak heap, RSS를 제한한다는 주장
- destructor가 process의 모든 종료 경로에서 반드시 실행된다는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — What Is Ownership?](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
- **언어 규범:** [Move expressions](https://doc.rust-lang.org/reference/expressions.html#moved-and-copied-types), [Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **구현 확인:** [`GlobalAlloc` safety notes](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html#safety)
