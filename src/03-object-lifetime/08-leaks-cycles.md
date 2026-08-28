# Leak과 reference cycle

Memory leak은 일반적으로 memory safety 위반이 아니다. 더 이상 필요하지 않은 resource가 계속 reachable하거나, destructor가 실행될 조건이 영원히 충족되지 않는 상태다. safe Rust에서도 만들 수 있다.

## Strong reference cycle

`Rc<T>`나 `Arc<T>`는 strong count가 0이 될 때 내부 값을 drop한다. 두 객체가 서로를 strong reference로 소유하면 count가 0이 되지 않는다.

```text
Node A --strong--> Node B
Node A <--strong-- Node B

external owner 제거
    ↓
A strong_count = 1
B strong_count = 1
    ↓
둘 다 Drop되지 않음
```

parent-child tree에서는 보통 parent가 child를 strong하게 소유하고, child의 parent link는 `Weak<T>`로 표현한다. `Weak`는 allocation을 가리킬 수 있지만 내부 값을 계속 살아 있게 하는 strong ownership은 제공하지 않는다.

```rust
use std::rc::{Rc, Weak};

struct Node {
    parent: Weak<Node>,
}

fn main() {
    let root = Rc::new(Node { parent: Weak::new() });
    assert!(root.parent.upgrade().is_none());
    assert_eq!(Rc::strong_count(&root), 1);
}
```

## Cycle이 없어도 leak처럼 보이는 성장

- global registry가 entry를 제거하지 않음
- cache에 eviction policy가 없음
- channel consumer보다 producer가 빨라 backlog가 증가
- completed task handle이나 metrics label이 계속 누적
- `Vec::clear` 후 capacity가 의도적으로 유지됨

앞의 네 항목은 live state의 논리적 성장이고, 마지막 항목은 collection retention이다. allocator retention과도 구분해야 한다.

## 탐지 질문

```text
live object count가 증가하는가?
  ├─ 예 → owner graph, cache/queue policy 확인
  └─ 아니요
       ↓
allocated bytes는 감소했는가?
  ├─ 아니요 → collection capacity / allocator retention 확인
  └─ 예
       ↓
RSS만 높은가?
  └─ allocator와 OS page 상태 확인
```

## 보장 경계

### 이 장이 보장하는 설명

- strong reference cycle이 destructor 실행을 막을 수 있다는 점
- `Weak`가 strong ownership을 추가하지 않는다는 점

### 이 장이 보장하지 않는 것

- 모든 memory growth가 reference cycle 때문이라는 주장
- `Weak`를 사용하면 cache/queue가 자동으로 bounded된다는 주장
- leak이 없으면 RSS가 즉시 baseline으로 돌아간다는 주장

### 출처와 권위

- **공식 학습서:** [Reference Cycles Can Leak Memory](https://doc.rust-lang.org/book/ch15-06-reference-cycles.html)
- **고급/unsafe:** [Rustonomicon — Leaking](https://doc.rust-lang.org/nomicon/leaking.html)
- **구현 확인:** [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html), [`Weak`](https://doc.rust-lang.org/std/rc/struct.Weak.html), [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html)
