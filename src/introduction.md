# 들어가며

Rust의 메모리 관리를 `ownership` 하나로 설명하면 중요한 절반을 놓친다. ownership은 누가 값을 소유하고 누가 접근할 수 있는지를 결정한다. 그러나 장시간 실행되는 서버가 memory limit 안에서 살아남는지는 collection의 capacity, allocator의 재사용 정책, Linux의 virtual memory, cgroup, 애플리케이션의 admission policy까지 함께 결정한다.

이 책은 다음 문장을 출발점으로 삼는다.

> **Rust의 ownership은 메모리 사용량을 관리하는 체계가 아니라, 값의 lifetime과 접근 권한을 관리하는 체계다.**

따라서 다음 두 질문은 분리해야 한다.

- **memory safety:** 이 reference를 역참조해도 유효한가? 같은 메모리에 충돌하는 접근이 있는가? 누가 `Drop`할 것인가?
- **memory capacity:** 이 작업이 몇 byte를 필요로 하는가? process와 cgroup의 여유는 얼마인가? 초과하면 대기, spill, 실패 중 무엇을 할 것인가?

## 읽는 순서

```text
값과 접근 권한
    ↓
객체가 파괴되는 시점
    ↓
heap allocation을 소유하는 type
    ↓
allocator의 allocate/deallocate
    ↓
OS의 virtual/physical memory
    ↓
cgroup의 reclaim/OOM
    ↓
server/DB의 memory governance
```

C/C++ 개발자는 RAII와 smart pointer라는 익숙한 발판을 이용하되, 대응 관계를 동일성으로 오해하지 않아야 한다. 예를 들어 `Box<T>`와 `std::unique_ptr<T>`는 단일 소유권이라는 점에서 비슷하지만, Rust의 move와 borrow checking은 C++의 move constructor나 관례적 pointer discipline보다 언어 전반에 더 강하게 연결된다.

## 이 책이 다루지 않는 것

- `unsafe` Rust 전체를 가르치는 완전한 지침
- 특정 allocator가 모든 workload에서 우수하다는 성능 결론
- 모든 Linux 배포판과 kernel version에 동일하게 적용되는 tuning 값
- 어떤 DB 엔진에도 그대로 복사할 수 있는 단일 memory pool 구현

이 책은 layer 사이의 계약과 실패 경로를 설명한다. 실제 한도와 정책 값은 workload 측정으로 결정해야 한다.

## 보장 경계

### 이 장이 보장하는 설명

- memory safety와 memory capacity가 서로 다른 문제라는 분류
- 뒤 장에서 사용할 layer와 용어의 일관된 순서

### 이 장이 보장하지 않는 것

- safe Rust 프로그램이 leak이나 OOM을 일으키지 않는다는 주장
- allocation 성공이 physical memory 확보를 뜻한다는 주장

### 출처와 권위

- **공식 학습서:** [Rust Book — What Is Ownership?](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
- **언어 규범:** [Rust Reference — Destructors](https://doc.rust-lang.org/reference/destructors.html)
- **OS 공식:** [Linux kernel — Overcommit Accounting](https://www.kernel.org/doc/html/latest/mm/overcommit-accounting.html)
