# 시스템 엔지니어를 위한 Rust 메모리 관리

**From ownership to OOM — C/C++ 개발자가 장시간 실행되는 Rust 서버를 이해하기 위한 교과서**

이 저장소는 일반적인 Rust 입문서가 아니다. Rust의 언어 규칙에서 출발해 collection과 heap, allocator, Linux virtual memory와 RSS, cgroup과 OOM, 마지막으로 server/DB memory governance까지 하나의 연속된 mental model로 연결한다.

> Rust의 ownership은 메모리 사용량을 제한하는 체계가 아니라, 값의 lifetime과 접근 권한을 관리하는 체계다.

## 이 문서를 만드는 이유

C++ 시스템에서는 모든 대상 allocation을 capped allocator에 통과시키고 `std::bad_alloc`을 task boundary에서 처리하는 구조를 만들 수 있다. 이때 admission은 workload 분배와 backpressure를 담당하고, allocator failure는 estimate 오차를 막는 마지막 recoverable boundary가 된다.

일반적인 Rust `std` 프로그램의 기본 경로는 다르다. `Vec::push`, `Box::new`, formatting이나 dependency 내부의 infallible allocation이 실패하면 보통 `handle_alloc_error`를 거쳐 process가 abort한다. `GlobalAlloc` 구현 자체도 현재 계약상 unwind할 수 없다.

따라서 Rust server/DB에서는 allocation failure를 기다리지 않고 그보다 앞에서 다음 invariant를 만들어야 한다.

```text
sum(active workload reservations) <= governed application budget
```

이 invariant는 total RSS를 예측하는 공식이 아니다. 관리 대상 workload의 **memory commitment 상한**이다. Heap 밖의 anonymous memory, allocator overhead/retention, stack, native allocation, page cache와 socket charge는 headroom, 관측, bounded arena, cgroup으로 별도 통제한다.

이 논지의 전체 설명은 [왜 allocation failure 대신 admission인가](src/01-mental-model/00-why-admission.md)에서 시작한다.

## 대상 독자

- C/C++의 pointer, RAII, `malloc`/`free`, `std::unique_ptr`에 익숙한 시스템 개발자
- Rust의 ownership과 borrowing은 배웠지만 RSS나 OOM과 어떻게 이어지는지 설명하기 어려운 개발자
- 장시간 실행되는 server, storage engine, database, runtime의 memory policy를 설계하는 개발자

## 범위

```text
Language
  ownership / borrowing / lifetime / move
      ↓
Object lifetime
  Drop / RAII / destructor scope
      ↓
Collections & heap
  Box / Vec / String / HashMap / Arc / capacity
      ↓
Allocator
  allocation / deallocation / fragmentation / retention
      ↓
OS virtual memory
  page / RSS / overcommit / page fault / OOM
      ↓
Resource control
  RFC 2116 / try_reserve / cgroup v2
      ↓
Server & DB governance
  budget / reservation / admission / spill / eviction / backpressure
```

## 학습 목표

이 교재를 마치면 다음을 설명할 수 있어야 한다.

1. ownership, borrowing, lifetime이 memory safety에 기여하는 방식과 그 보장 경계
2. `Box`, `Vec`, `String`, `HashMap`, `Arc`가 언제 heap allocation을 일으키는지
3. `Drop`과 deallocation, allocator 반환과 OS RSS 감소가 서로 다른 사건인 이유
4. `reserve`와 `try_reserve`의 차이, RFC 2116의 설계 의도와 현재 stable Rust의 상태
5. Linux overcommit 때문에 `try_reserve` 성공 후에도 OOM kill이 가능한 이유
6. cgroup limit과 application memory budget을 함께 사용해야 하는 이유
7. server/DB에서 memory pool, admission, spill, eviction, backpressure를 배치하는 방법

## 가장 먼저 읽을 장

[왜 allocation failure 대신 admission인가](src/01-mental-model/00-why-admission.md)는 C++의 recoverable `std::bad_alloc` 모델과 Rust의 기본 abort 경로를 비교하고, admission이 보장하는 정확한 상한을 정의한다. 이어서 [Rust가 막는 문제와 막지 않는 문제](src/01-mental-model/02-safety-boundary.md)는 `use-after-free`, `data race`, leak, allocator retention, RSS, process OOM, cgroup OOM kill의 경계를 한 표로 정리한다.

## 출처 정책

각 장은 가능한 한 다음 범주를 구분해 표시한다.

| 표시 | 역할 |
|---|---|
| **공식 학습서** | Rust Book. 개념을 배우는 첫 출처 |
| **언어 규범** | Rust Reference. 언어가 정의하는 동작을 확인하는 출처 |
| **구현 확인** | `rust-lang/rust` source와 standard library API 문서 |
| **설계 배경** | 승인된 RFC. 당시 목표와 trade-off를 설명하지만 현재 구현과 같다고 가정하지 않음 |
| **고급/unsafe** | Rustonomicon과 Unsafe Code Guidelines. unsafe semantics와 미확정 영역을 구분 |
| **OS 공식** | Linux kernel, cgroup, proc 문서 |
| **보조 학습** | Rust Performance Book, High Assurance Rust 등 비규범적 해설 |

상세 원칙은 [출처와 권위 읽는 법](src/source-policy.md), 전체 목록은 [참고 문헌](src/references.md)에 있다. 각 장의 마지막에는 **무엇을 보장하는가 / 무엇을 보장하지 않는가**를 적어 공식 보장, 구현 관찰, 운영 정책을 분리한다.

## 다이어그램 정책

다이어그램은 `diagram-design`의 구조·접근성 검사를 적용한 standalone HTML/SVG로 작성한다. 색과 서체는 Rust 공식 사이트에서 확인한 token을 따르며, 추출 근거와 의도적인 차이는 [Rust brand fidelity receipt](diagrams/BRAND_FIDELITY.md)에 기록한다.

## 로컬에서 읽고 검증하기

```bash
cargo install mdbook --version 0.5.4 --locked
mdbook test
cargo test --workspace --all-targets
mdbook build
```

생성된 사이트는 `book/index.html`에서 확인할 수 있다. `main`에 push하면 GitHub Actions가 같은 검증을 수행한 뒤 GitHub Pages에 배포한다.

## Git identity 안전장치

이 저장소의 commit과 push는 개인 identity만 허용한다.

```text
GitHub account : Tangeroooo
commit name    : Tangeroooo
commit email   : juhyeon113@gmail.com
```

`scripts/install-hooks.sh`는 위 값을 **repo-local Git config**에만 기록하고 `.githooks/pre-commit`, `.githooks/pre-push`를 활성화한다. 전역 Git 설정은 변경하지 않는다. `pre-push`는 현재 `gh` 로그인 계정, remote owner, push 대상 commit의 author/committer를 다시 확인한다.

## 프로젝트 상태와 라이선스

이 자료는 학습용 초판이다. Rust와 Linux의 버전별 동작은 연결된 공식 문서를 우선한다. 별도의 라이선스가 아직 부여되지 않았으므로, 저작권법상 허용되는 범위를 넘는 재배포나 2차 저작은 명시적 허가가 필요하다.
