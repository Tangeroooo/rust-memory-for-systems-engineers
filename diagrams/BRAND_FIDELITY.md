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
