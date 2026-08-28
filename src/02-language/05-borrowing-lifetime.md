# Borrowing과 lifetime

Borrowing은 ownership을 이전하지 않고 reference를 통해 값에 접근하는 방식이다. Lifetime은 값의 “보관 기간을 늘리는 장치”가 아니라 reference가 유효해야 하는 관계를 compiler가 검사할 수 있도록 표현한다.

## Shared borrow와 mutable borrow

```rust
fn bytes(value: &String) -> usize {
    value.len()
}

fn append_suffix(value: &mut String) {
    value.push_str(" management");
}

fn main() {
    let mut title = String::from("memory");
    assert_eq!(bytes(&title), 6);
    append_suffix(&mut title);
    assert_eq!(title, "memory management");
}
```

학습 모델은 다음과 같다.

- `&T`: 여러 shared reference가 읽을 수 있다.
- `&mut T`: 해당 access 동안 exclusive한 mutable reference다.
- reference는 대상 값을 소유하지 않으므로 reference가 끝난다고 대상이 drop되지는 않는다.

정확한 aliasing semantics의 모든 세부사항을 단순한 slogan으로 대체해서는 안 된다. 특히 raw pointer와 `UnsafeCell`이 들어가는 `unsafe` code는 별도의 invariant가 필요하다.

## Lifetime annotation이 하는 일

```rust
fn choose_first<'a>(left: &'a str, _right: &str) -> &'a str {
    left
}

fn main() {
    let left = String::from("left");
    let right = String::from("right");
    assert_eq!(choose_first(&left, &right), "left");
}
```

`'a`는 반환 reference가 `left`의 유효 범위를 넘을 수 없다는 관계를 나타낸다. `'a`가 `left`의 allocation을 연장하거나 allocator에 무엇을 요청하는 것은 아니다.

## C++ reference와 비교할 때의 함정

C++에서도 reference와 pointer의 유효성을 올바르게 관리할 수 있다. 차이는 Rust의 safe subset이 그 규칙을 type checking의 중심에 둔다는 점이다. 반대로 `unsafe` Rust나 FFI boundary에서는 programmer가 safe abstraction이 기대하는 invariant를 다시 책임져야 한다.

## 보장 경계

### 이 장이 보장하는 설명

- safe reference가 사용되는 동안 유효한 값을 가리켜야 한다는 점
- lifetime parameter가 reference 사이의 유효성 관계를 표현한다는 점

### 이 장이 보장하지 않는 것

- lifetime annotation이 object나 allocation을 runtime에서 연장한다는 주장
- borrow checking이 deadlock, logical race, unbounded queue를 방지한다는 주장
- 아직 완전히 규범화되지 않은 모든 unsafe aliasing 세부사항

### 출처와 권위

- **공식 학습서:** [References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html), [Validating References with Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
- **언어 규범:** [Reference types](https://doc.rust-lang.org/reference/types/pointer.html#shared-references-), [Undefined behavior](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)
- **고급/unsafe:** [Unsafe Code Guidelines glossary](https://rust-lang.github.io/unsafe-code-guidelines/glossary.html)
