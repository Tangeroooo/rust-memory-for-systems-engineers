# 출처와 권위 읽는 법

문서의 출처는 모두 같은 종류의 보장을 제공하지 않는다. 학습서의 친절한 설명, Reference의 언어 규칙, RFC의 과거 제안, 현재 source code의 구현 세부사항을 구분해야 한다.

## 권위 범주

| 범주 | 이 책에서의 의미 | 사용할 때의 주의점 |
|---|---|---|
| **언어 규범** | Rust Reference가 정의하는 syntax와 semantics | Rust Reference도 스스로 불완전할 수 있음을 밝힌다. “ISO 표준”과 같은 완결된 형식 명세로 과장하지 않는다. |
| **공식 학습서** | Rust Book의 안정된 학습 설명 | 이해의 출발점이지 allocator·OS 운영 보장의 출처는 아니다. |
| **구현 확인** | standard library API 문서와 `rust-lang/rust` source | 공개 API의 documented guarantee와 현재 내부 구현을 구분한다. 내부 함수와 growth policy는 바뀔 수 있다. |
| **설계 배경** | 승인된 RFC가 기록한 문제, 선택지, rationale | RFC의 모든 제안이 현재 stable Rust에 그대로 구현되었다고 가정하지 않는다. |
| **고급/unsafe** | Rustonomicon, Unsafe Code Guidelines | Rustonomicon은 고급 지침이지만 draft 성격이 있다. UCG의 미확정 논의는 normative guarantee가 아니다. |
| **OS 공식** | Linux kernel/cgroup/VM 문서 | kernel version, cgroup v1/v2, container runtime 설정을 함께 확인한다. |
| **보조 학습** | Rust Performance Book, High Assurance Rust | 관측법과 mental model을 보강하지만 Rust 언어의 규범적 출처는 아니다. |

## 주장 표기 원칙

각 장은 세 부분으로 끝난다.

1. **이 장이 보장하는 설명:** 공식 문서나 공개 API에서 직접 뒷받침되는 내용
2. **이 장이 보장하지 않는 것:** 흔히 한 단계 더 확대 해석하는 잘못된 결론
3. **출처와 권위:** 해당 주장을 확인할 수 있는 원문과 그 역할

문장에서 다음 표현을 구분한다.

- “Rust가 보장한다” — Reference 또는 public API documentation에 명시된 계약
- “현재 standard library 구현은” — `rust-lang/rust` source에서 확인한 구현 세부사항
- “RFC는 제안했다” — 설계 당시 제안이며 현재 상태는 별도 확인
- “운영 정책으로 권장한다” — 이 책의 설계 권고이며 언어 보장이 아님

## 버전과 날짜

이 책은 특정 snapshot의 내부 구현에 의존하지 않도록 public guarantee를 우선한다. 현재 상태를 언급할 때는 stable API 문서와 Cargo 문서를 함께 확인한다. Linux의 동작은 배포 환경에서 다음을 다시 확인한다.

```text
kernel version
cgroup version
vm.overcommit_memory
swap 구성
container memory limit
allocator 종류와 설정
```

## 보장 경계

### 이 장이 보장하는 설명

- 독자가 의견, 설계 배경, 현재 구현, 언어 보장을 구분할 수 있는 표기 체계
- RFC 2116 같은 역사적 문서를 현재 API와 대조하는 방법

### 이 장이 보장하지 않는 것

- 높은 권위 범주의 문서가 모든 질문에 답한다는 주장
- 현재 source code의 모든 세부사항이 향후 version에서도 유지된다는 주장

### 출처와 권위

- **언어 규범:** [Rust Reference — Introduction](https://doc.rust-lang.org/reference/introduction.html)
- **공식 학습서:** [The Rust Programming Language](https://doc.rust-lang.org/book/)
- **한국어 용어/문체 참고:** [한국 Rust 사용자 그룹의 Rust Book 번역](https://doc.rust-kr.org/)
- **구현 확인:** [`rust-lang/rust`](https://github.com/rust-lang/rust)
- **설계 배경:** [`rust-lang/rfcs`](https://github.com/rust-lang/rfcs)
