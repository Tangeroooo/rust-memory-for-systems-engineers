# Allocation과 deallocation

Allocator는 `Layout`으로 표현된 size와 alignment의 memory block을 제공하고 회수한다. Rust language의 ownership rule과 allocator의 block management는 연결되지만 같은 subsystem은 아니다.

## 기본 경로

```text
Box / Vec / String / HashMap
          ↓
alloc crate의 collection 구현
          ↓
Global allocator
          ↓
등록된 #[global_allocator] 또는 std의 기본 선택
          ↓
system allocator / custom allocator
          ↓
OS virtual memory primitives
```

`std::alloc` 문서는 한 프로그램에 `Box`와 `Vec` 등이 사용하는 global allocator가 있음을 설명한다. 일반 executable에서 std의 정확한 기본 allocator 선택은 public contract로 고정되어 있지 않다. 특정 allocator를 전제로 tuning하려면 build와 deployment 설정에서 명시하고 검증해야 한다.

## Allocation과 initialization

Allocation은 addressable byte block을 얻는 일이다. 그 byte가 유효한 `T`로 초기화되었다는 뜻은 아니다. Safe collection은 initialized element 수를 `len`으로 추적하고 spare capacity를 직접 읽지 못하게 한다.

```text
allocate Layout
    ↓
uninitialized storage
    ↓ write valid T
initialized value
    ↓ run destructor
logically dead value
    ↓ deallocate Layout
block returned to allocator
```

이 경계를 직접 다루는 `MaybeUninit`, raw pointer, `alloc`/`dealloc`은 `unsafe` invariant가 필요하다.

## Deallocation의 정확한 의미

Collection이 buffer를 해제하면 allocator의 `deallocate` 계약으로 block을 돌려준다. 이후 같은 pointer를 다시 사용하면 안 된다. 그러나 allocator는 그 virtual memory를 다음 allocation에 재사용할 수 있도록 process 안에 보관할 수 있다.

```text
Rust object가 buffer ownership을 종료
  → allocator에는 free block
  → OS에는 여전히 mapped/resident page일 수 있음
```

## Allocation failure 경로

Low-level allocator API는 failure를 표현할 수 있다. 반면 `Box::new`, `Vec::push`, `String` growth 같은 일반 API는 사용자가 `Result`를 처리하는 형태가 아니다. Standard library의 `handle_alloc_error`는 `std`를 링크하는 기본 구성에서 메시지를 출력하고 process를 abort한다. Fallible collection API는 뒤의 RFC 2116 장에서 다룬다.

## 보장 경계

### 이 장이 보장하는 설명

- Rust collection에서 global allocator로 이어지는 개념적 경로
- deallocation 후 pointer를 더 이상 사용할 수 없다는 allocator 계약

### 이 장이 보장하지 않는 것

- executable의 기본 allocator가 특정 구현으로 영구 고정된다는 주장
- deallocation이 즉시 `munmap`이나 RSS 감소로 이어진다는 주장
- source에 적힌 모든 allocation이 optimizer 이후에도 실제 allocator 호출로 남는다는 주장

### 출처와 권위

- **구현 확인/public contract:** [`std::alloc`](https://doc.rust-lang.org/std/alloc/index.html), [`GlobalAlloc`](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html), [`Allocator`](https://doc.rust-lang.org/std/alloc/trait.Allocator.html)
- **구현 확인/source:** [`RawVec`](https://github.com/rust-lang/rust/blob/main/library/alloc/src/raw_vec/mod.rs)
- **고급/unsafe:** [Unsafe Code Guidelines — Allocation](https://rust-lang.github.io/unsafe-code-guidelines/glossary.html#allocation)
