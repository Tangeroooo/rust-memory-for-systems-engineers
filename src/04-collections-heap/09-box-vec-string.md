# Box, Vec, String의 표현

Rust type을 보고 allocation behavior를 추론하려면 **값의 inline 부분**과 **별도 allocation을 소유하는 부분**을 나눠 보아야 한다.

## `Box<T>`

`Box<T>`는 `T`를 heap에 두고 그 allocation의 ownership을 가진다. 다만 zero-sized type은 실제 allocation이 필요하지 않을 수 있다. `Box` 값의 move는 내부 `T`를 논리적으로 move하는 ownership transfer이지 `T`의 heap bytes를 새 allocation으로 복사하는 `clone`이 아니다.

```rust
fn main() {
    let first = Box::new([1_u8, 2, 3, 4]);
    let second = first;
    assert_eq!(*second, [1, 2, 3, 4]);
}
```

## `Vec<T>`

표준 `Vec<T>`의 public guarantee는 pointer, capacity, length의 triplet이다. Field 순서는 지정되지 않는다.

```text
Vec value
┌─────────┬──────────┬────────┐
│ pointer │ capacity │ length │
└────┬────┴──────────┴────────┘
     │
     ▼ heap allocation
┌───────────────┬────────────────────────┐
│ len initialized T values │ spare capacity │
└───────────────┴────────────────────────┘
```

`len <= capacity`다. `push` 시 `len < capacity`이면 재allocation하지 않고, `len == capacity`이면 재allocation이 필요하다. Growth factor는 public contract가 아니다.

```rust
fn main() {
    let mut values = Vec::with_capacity(4);
    let initial_capacity = values.capacity();

    for value in 0..4 {
        values.push(value);
    }

    assert!(initial_capacity >= 4);
    assert_eq!(values.len(), 4);
    assert_eq!(values.capacity(), initial_capacity);
}
```

## `String`

`String`은 growable UTF-8 encoded buffer다. Capacity는 byte 단위다. 문자 수, Unicode scalar value 수, grapheme cluster 수와 같지 않다.

```rust
fn main() {
    let value = String::from("메모리");
    assert_eq!(value.len(), 9); // UTF-8 bytes
    assert_eq!(value.chars().count(), 3);
}
```

문자열 concatenation, formatting, `to_string`, `clone`은 새 allocation 또는 growth를 일으킬 수 있다. API 이름만 보고 단정하지 말고 capacity 변화와 profiling 결과를 함께 본다.

## Stack과 heap이라는 이분법의 한계

`Vec<String>`은 최소 두 층의 allocation을 가질 수 있다.

```text
Vec<String> buffer
  ├─ String header 0 ──→ UTF-8 buffer 0
  ├─ String header 1 ──→ UTF-8 buffer 1
  └─ String header 2 ──→ UTF-8 buffer 2
```

따라서 `Vec` 하나의 capacity만 측정해서 전체 heap footprint를 구할 수 없다. Element가 다시 allocation을 소유하는지 확인해야 한다.

## 보장 경계

### 이 장이 보장하는 설명

- 표준 `Vec<T>`의 pointer/capacity/length 모델과 initialized/spare 영역
- `String::len`과 capacity가 byte 단위라는 점

### 이 장이 보장하지 않는 것

- `Vec` field 순서나 growth factor
- `Vec<T>` capacity만으로 nested allocation까지 계산할 수 있다는 주장
- source에 보이는 모든 `Box::new`가 최종 machine code에서 관측 가능한 allocator 호출이 된다는 주장

### 출처와 권위

- **구현 확인/public contract:** [`Box`](https://doc.rust-lang.org/std/boxed/struct.Box.html), [`Vec` guarantees](https://doc.rust-lang.org/std/vec/struct.Vec.html#guarantees), [`String`](https://doc.rust-lang.org/std/string/struct.String.html)
- **공식 학습서:** [Using `Box<T>` to Point to Data on the Heap](https://doc.rust-lang.org/book/ch15-01-box.html)
- **구현 확인/source:** [`alloc::vec`](https://github.com/rust-lang/rust/blob/main/library/alloc/src/vec/mod.rs), [`alloc::string`](https://github.com/rust-lang/rust/blob/main/library/alloc/src/string.rs)
