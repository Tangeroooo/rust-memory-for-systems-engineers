# 용어집

이 책은 한국 Rust 사용자 그룹의 Rust Book 번역이 사용하는 친절한 문체를 참고하되, 시스템 분야에서 뜻이 갈릴 수 있는 keyword는 영어를 함께 적는다.

| 용어 | 이 책에서의 뜻 |
|---|---|
| 소유권(ownership) | 값의 owner와 정리 책임을 정하는 언어 규칙 |
| 빌림(borrowing) | ownership을 이전하지 않고 reference로 접근하는 것 |
| 참조자(reference) | 유효한 값에 대한 shared 또는 mutable reference. Raw pointer와 구분 |
| 라이프타임(lifetime) | 주로 reference가 유효해야 하는 관계를 나타내는 개념과 표기 |
| 이동(move) | 값의 ownership이 새 place로 이전되는 semantic event |
| `Drop` | destructor 동작을 사용자 정의하는 trait와 값의 파괴 과정 |
| 할당(allocation) | allocator에서 `Layout`에 맞는 memory block을 얻는 일 |
| 해제(deallocation) | 이전에 할당된 block을 allocator에 반환하는 일 |
| allocator | allocation, growth, shrink, deallocation을 관리하는 구성 요소 |
| 길이(length, `len`) | collection 안의 논리적으로 initialized된 element/byte 수 |
| 용량(capacity) | 재allocation 없이 보관 가능한 element/byte 수 |
| `reserve` | 추가 capacity를 infallible model로 확보하는 collection API |
| `try_reserve` | 추가 capacity 확보 실패를 `TryReserveError`로 반환할 수 있는 API |
| 단편화(fragmentation) | free/unused space가 block 내부나 block 사이에 흩어져 효율이 낮아지는 현상 |
| 보유(retention) | collection이나 allocator가 재사용을 위해 memory를 계속 보관하는 현상 |
| 가상 메모리(virtual memory) | process가 보는 virtual address와 mapping 체계 |
| RSS(resident set size) | 현재 RAM에 resident한 process page의 양을 나타내는 OS 관측값 |
| overcommit | 미래 physical backing보다 큰 virtual/committed memory 요청을 허용할 수 있는 정책 |
| OOM(out of memory) | memory 부족 상태의 총칭. allocator failure, abort, global/cgroup kill을 구분해야 함 |
| anonymous memory | filesystem의 file로 backing되지 않은 mapping/page. Heap, stack, anonymous `mmap`, private Copy-on-Write page 등을 포함 |
| admission | 작업을 시작하거나 grow할 memory commitment를 승인, 대기, 거절하는 application policy |
| reservation | 작업에 부여한 논리적 memory 사용 권한. Physical page의 예약과 구분 |
| charge | 관리 대상 state의 실제 growth를 application ledger에 반영한 값 |
| headroom | allocator overhead, stack, native/direct mapping 등 governed charge 밖의 사용량과 변동을 흡수하는 여유 |
| memory governance | budget, reservation, admission, spill, eviction, backpressure를 통한 application-level 사용량 제어 |

## 번역하지 않고 병기하는 이유

`lifetime`, `Drop`, `allocator`, `RSS`, `OOM`은 Rust code, API 문서, Linux 운영 문서에서 그대로 검색해야 하는 keyword다. 최초 등장과 오해 가능성이 있는 문맥에서는 한국어 설명을 붙이고, 이후에는 원어를 유지한다.

## 보장 경계

### 이 장이 보장하는 설명

- 이 책 안에서 사용하는 용어의 일관된 의미

### 이 장이 보장하지 않는 것

- 모든 한국어 Rust 문서가 동일한 번역어를 사용한다는 주장
- 일상적인 “수명”과 Rust lifetime parameter가 완전히 같은 개념이라는 주장

### 출처와 권위

- **한국어 용어/문체 참고:** [한국 Rust 사용자 그룹의 Rust Book 번역](https://doc.rust-kr.org/), [`rust-kr/doc.rust-kr.org`](https://github.com/rust-kr/doc.rust-kr.org)
- **영문 원문:** [The Rust Programming Language](https://doc.rust-lang.org/book/)
