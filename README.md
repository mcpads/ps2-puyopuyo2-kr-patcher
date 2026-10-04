# 뿌요뿌요 2 퍼펙트 셋 (PlayStation 2) 한글 패처

PlayStation 2 일본판 《SEGA AGES 2500 시리즈 Vol.12 뿌요뿌요 2 퍼펙트 셋》(SLPM-62400) 디스크 이미지에 한글 패치를 적용하는 Rust 코드입니다. 원본 BIN/CUE 검증, MODE2/2352 섹터와 EDC/ECC 재생성, ISO 9660 읽기, CPX 압축·해제와 PSMT8 텍스처 변환, R5900 실행 파일 조사와 typed assembly 패치, 한글 글리프 배정과 래스터화, Expected Write 기반 누적 빌드, xdelta 패키지 생성·적용을 제공합니다.

배포용 패치와 적용 방법은 [뿌요뿌요 시리즈 한글 번역 프로젝트](https://github.com/mcpads/puyo-puyo-kr-patch/tree/main/ps2-puyopuyo2-perfect-set)에서 제공합니다.

## 제공하지 않는 것

이 저장소에는 원본 디스크, 패치를 적용한 디스크, 번역 원장(`assets/translation/`), 그래픽 번역 영역 원장(`config/static-graphics-regions.json`), 번역 그래픽과 폰트 파일이 없습니다. 따라서 이 저장소만으로는 배포 패치를 다시 만들 수 없습니다. 아래 입력을 모두 갖춘 경우에만 `build-technical-translation-development`가 한글 디스크를 생성합니다.

## 빌드와 테스트

```bash
cargo build --locked
cargo test --locked
```

Rust 1.96.0(`rust-toolchain.toml`)을 사용합니다. 기본 테스트는 합성 입력만 사용합니다. 번역 그래픽 원장이나 그래픽 영역 원장이 필요한 테스트는 `#[ignore = "requires ..."]`로 필요한 입력을 밝혀 두었습니다. 입력을 저장소 루트의 같은 경로에 둔 뒤 `cargo test --locked -- --ignored`로 실행하며, 입력이 없으면 성공으로 넘어가지 않고 실패합니다.

## 지원 원본

| 파일 | 크기 | SHA-256 |
| --- | --- | --- |
| `Sega Ages 2500 Series Vol. 12 - Puyo Puyo Tsuu - Perfect Set (Japan).bin` | 592,243,008바이트 | `93c8005f0a2ed83659307234b3298b3e8b3c0a78af3cd27a784fd26fc9d95ed4` |
| `Sega Ages 2500 Series Vol. 12 - Puyo Puyo Tsuu - Perfect Set (Japan).cue` | 134바이트 | `256731a1da3fdf8c5e1668cadbbea20257f22ecf3cc311115b3d324e564416f8` |

원본 계약은 `config/source.json`에 있으며 모든 원본 의존 명령이 BIN/CUE의 크기와 SHA-256을 먼저 검사합니다. 다른 디스크는 거부합니다.

## 빌드 입력

| 입력 | 기본 경로 | 비고 |
| --- | --- | --- |
| 원본 BIN/CUE | `--bin`, `--cue`로 지정 | 위 표와 일치해야 함 |
| 텍스처 미리보기 보고서 | `out/research/texture-previews.json` | 원본에서 `render-texture-previews`로 생성 |
| 번역 원장 | `assets/translation/` | 메모리 카드·실행 파일 문구·승인 용어·그래픽·스토리 대사 (미포함) |
| 그래픽 번역 영역 원장 | `config/static-graphics-regions.json` | 미포함 |
| 공용 UI 폰트 | `--shared-font` | 배달의민족 주아체 `BMJUA.ttf` ([BMJUA](https://github.com/fonts-archive/BMJUA)) |
| 스토리 대사 폰트 | `--story-font` | 메이플스토리 서체 `Maplestory Light.ttf` ([메이플스토리 서체](https://maplestory.nexon.com/Media/Font)) |
| xdelta3 | `PATH` 또는 `--xdelta` | 패키지 생성·적용에만 필요 |

폰트는 재배포 조건을 이 저장소에서 보장할 수 없어 포함하지 않습니다. 각 폰트의 라이선스는 배포처에서 확인하세요. 빌드는 `config/poc-font.json`과 `config/story-dialogue-font.json`에 고정한 SHA-256과 다른 폰트를 거부합니다. 배포 패치 v0.1.0은 다음 폰트 파일로 만들었습니다.

```text
e8e6aa8b1b662c7bf0d7f136f29e822e0985176458a6e5d0ba08afc4a5c901a9  BMJUA.ttf
6d51d8e576f77b01914095aa1f69f9d37c16d93fe940d748962867f218442ba9  Maplestory Light.ttf
```

번역 그래픽을 다시 렌더하는 명령은 이 밖에 Galmuri 등 `config/*-font.json`에 적힌 폰트를 요구합니다. 최종 디스크 빌드는 이미 렌더된 번역 그래픽을 읽으므로 위 두 폰트만 필요합니다.

## 디스크와 패치 생성

```bash
ROM="roms/Sega Ages 2500 Series Vol. 12 - Puyo Puyo Tsuu - Perfect Set (Japan)"

cargo run --locked -- render-texture-previews \
  --cue "$ROM.cue" --bin "$ROM.bin" \
  --output-directory out/research/texture-previews \
  --report out/research/texture-previews.json

cargo run --locked -- build-technical-translation-development \
  --cue "$ROM.cue" --bin "$ROM.bin" \
  --shared-font path/to/BMJUA.ttf \
  --story-font "path/to/Maplestory Light.ttf"

cargo run --locked -- create-patch-package \
  --cue "$ROM.cue" --bin "$ROM.bin" \
  --target-bin out/development/technical-translation/puyo2-ko-technical-dev.bin \
  --build-report out/development/technical-translation/build.json \
  --output-directory out/package/v0.1.0
```

두 번째 명령은 `out/development/technical-translation/`에 `puyo2-ko-technical-dev.bin`, `.cue`와 빌드 보고서 `build.json`을 씁니다. v0.1.0 배포 패치와 같은 입력이면 BIN SHA-256은 `7458ea05898ac8ab6ca9e7a189a6b28df431f0d449c093b2f91537cd202e996c`입니다. 세 번째 명령은 xdelta3로 `translation.xdelta`를 만들고 원본에 다시 적용해 목표 해시를 확인합니다. 만든 패키지는 `apply-patch --package <디렉터리> --cue <원본.cue> --bin <원본.bin> --output-directory <새 디렉터리>`로 적용할 수 있습니다.

## 그 밖의 명령

원본 조사(`survey-*`), 텍스처 렌더(`render-*`), 번역 원장 생성·검사(`init-*`, `check-*`)와 구성요소별 빌드 명령의 목록은 `cargo run --locked -- --help`, 사용법은 `cargo run --locked -- help <명령>`으로 확인할 수 있습니다.

## 라이선스

이 저장소의 소스 코드는 [MIT License](LICENSE)로 제공합니다. R5900 typed ISA와 Expected Write는 MIT로 공개된 [retro-typed-isa](https://github.com/mcpads/retro-typed-isa)를 Cargo 의존 관계로 사용합니다.
