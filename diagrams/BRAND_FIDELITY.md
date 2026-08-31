# Rust brand fidelity receipt

확인일: 2026-08-28

이 저장소의 다이어그램은 `diagram-design` skill의 URL onboarding 절차에 따라 Rust 공식 사이트와 공식 Rust Book의 computed style을 확인한 뒤 제작했다. 전역 skill theme은 변경하지 않았으며, 아래 token은 이 저장소의 HTML 다이어그램에만 적용한다.

## 확인한 공식 화면

- [Rust Programming Language](https://www.rust-lang.org/)
- [The Rust Programming Language](https://doc.rust-lang.org/book/)

## 추출한 token

| 역할 | 값 | 공식 화면에서의 사용 |
|---|---|---|
| 본문 ink | `#2a3439` | Rust 공식 사이트 navigation, footer |
| Rust red | `#a72145` | 참여 영역, secondary button |
| Rust yellow | `#ffc832` | primary CTA, footer link |
| Purple | `#2e2459` | 학습 자료 영역 |
| Green | `#0b7261` | “Why Rust?” 영역 |
| Display type | `Alfa Slab One` | 공식 사이트 `h1` |
| Body type | `Fira Sans` | 공식 사이트 본문과 navigation |
| Code type | `Source Code Pro` | 공식 Rust Book code |

## 이 교재에 적용한 방식

- 배경은 pure white 대신 `diagram-design` onboarding의 reading-surface 규칙에 따라 warm paper `#fafaf7`을 사용한다.
- 기본 text와 connector는 `#2a3439`을 사용한다.
- 강조색은 diagram마다 Rust red와 Rust yellow 두 색 이하로 제한한다.
- 영문 display에는 `Alfa Slab One`, 본문에는 `Fira Sans`를 우선한다.
- 한국어 glyph는 `Noto Sans KR`, `Apple SD Gothic Neo`, `Malgun Gothic` 순으로 fallback한다.
- font는 Google Fonts의 공식 배포본을 불러오며, network가 없을 때는 system fallback으로 의미와 layout을 유지한다.

## 의도적인 차이

Rust 공식 사이트는 white background와 큰 display typography를 사용한다. 이 교재는 조밀한 기술 도해를 읽는 용도이므로 warm paper, 작은 heading scale, 얇은 border를 사용한다. 색과 type family는 공식 화면에 맞추되, layout density는 문서형 diagram에 맞게 조정했다.

## 2026-08-31 추가: tracking-comparison

- 기존에 추출한 project-local token을 그대로 재사용했다. 설치된 skill이나 전역 profile은 수정하지 않았다.
- `diagram-design` 2.6.11, nested containment, `doc-wide` 1280×720, static HTML이다. 사용자의 요청에 따라 호출 흐름도에서 영역·포함 관계 중심으로 수정했다.
- Process → allocator backing → 관리 대상 storage의 3단계 포함 관계를 좌우로 비교한다. 각 process의 task charge 면 두 곳에만 Rust red를 사용했다.
- C++ capped allocator와 Rust의 **명시적 fallible wrapper**를 비교한다. Rust의 기본 infallible failure path는 footer에서 별도로 밝힌다.
- Spare capacity, cap 밖 allocation/간접비용, allocator 바깥 stack·직접 mmap을 구획으로 표현했다. 실행 코드와 file-backed mapping 등은 생략하고 본문에서 이를 밝혔다. 면적은 실제 byte 비율이나 address layout을 의미하지 않는다.
- Source Code Pro는 code label, Fira Sans는 본문, Alfa Slab One은 영문 display에 사용했다. 한국어는 기존의 명시적 fallback을 유지한다.
