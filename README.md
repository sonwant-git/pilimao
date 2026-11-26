# Pilimao (霹靂貓播放器)

**Windows 10/11 原生 + Rust 音樂播放器** — 致敬經典 Winamp 2.x 的極簡三視窗操作體驗與現代高解析無失真音訊管線。

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

## 架構概覽（8-Crate Cargo Workspace）

| Crate | 核心職責 |
| --- | --- |
| `pilimao-dsp` | 64-bit (`f64`) 內部精度 10 段經典 EQ（60Hz–16kHz + Preamp，平滑防拉鍊雜訊）、Soft-Knee Limiter、Clickless Raised-Cosine Ramp、TPDF Dither（16/24-bit）、ReplayGain 2.0、`rubato` Sinc 重取樣、`rustfft` 19 段頻譜與示波器、Bit-Perfect 直通偵測 |
| `pilimao-decoder` | 基於 `symphonia` 的多格式解碼（MP3 / FLAC / OGG Vorbis / WAV / AAC / MP4 / ALAC / AIFF）與零靜音縫隙 **Gapless** 銜接播放 |
| `pilimao-metadata` | 基於 `lofty` 的標籤與 ReplayGain 2.0 / Opus R128 解析，內建舊版 **Big5 / GBK / Shift-JIS / UTF-8-in-Latin1** 亂碼自動還原（同時保留西歐拉丁重音字元） |
| `pilimao-playlist` | `.m3u` / `.m3u8` / `.pls` 讀寫、Winamp 二進位 `.eqf` / `winamp.q1` 讀寫、洗牌（含歷史回溯）、待播佇列（`Q`）、`nucleo-matcher` 模糊跳播（`J`） |
| `pilimao-skin-wsz` | 寬鬆 `.wsz` / 資料夾面板載入器、`region.txt` / `viscolor.txt` / `pledit.txt` 解析、100% Clean-Room 程式化預設面板、三視窗（Main / EQ / PL）磁吸與 **BFS 群組聯動 / Windowshade** 幾何引擎 |
| `pilimao-audio-engine` | `rtrb` 無鎖環形緩衝 + `triple_buffer` 無鎖 FFT 分流、Windows **WASAPI Shared / Exclusive（Bit-Perfect）+ MMCSS `"Pro Audio"`** 輸出與跨平台 `NullSink` |
| `pilimao-win32-shell` | `1x / 2x / 3x` Nearest-Neighbor 整數像素 BGRA 繪製器與 Win32 無邊框三視窗（`SetWindowRgn` + `BeginDeferWindowPos` + `WM_DROPFILES` + 經典快捷鍵） |
| `pilimao-app` | 支援 **Portable Mode**（`portable.ini`）的主程式與 CLI 檢測工具（`self-test` / `inspect-skin` / `inspect-eqf` / `inspect-track`） |

## 經典 Winamp 2.x 快捷鍵

| 快捷鍵 | 功能 |
| --- | --- |
| `Z` / `X` / `C` / `V` / `B` | 上一首 / 播放 / 暫停 / 停止 / 下一首 |
| `←` / `→` | 快退 / 快進 5 秒 |
| `↑` / `↓` | 音量 +5% / -5% |
| `J` | Jump-to-File 模糊搜尋跳播（`nucleo-matcher`） |
| `Q` | 加入 / 移出待播佇列（顯示 `[1]` 標記） |
| `S` / `R` | 切換洗牌 / 循環模式（Off → All → One） |
| `Alt+G` / `Alt+E` | 顯示 / 隱藏等化器（EQ）或播放清單（PL）視窗 |
| `Ctrl+D` | 切換 `1x` / `2x` / `3x` Nearest-Neighbor 整數縮放 |
| `Ctrl+W` | 切換目前視窗的 Windowshade 捲簾模式（自動推擠下方吸附視窗） |

## Portable Mode（免安裝可攜模式）

只要在 `pilimao.exe` 同目錄下放置空白的 `portable.ini`（或加上 `--portable` 參數），Pilimao 會自動將所有設定與狀態保存在執行檔旁邊，不寫入系統登錄檔或 `%APPDATA%`：

- `pilimao.ini`：縮放倍率、音量、平衡、輸出模式、視窗位置與 EQ 狀態
- `pilimao.eqf`：Winamp 相容二進位 EQ 預設
- `pilimao.m3u8`：最後播放清單
- `skins/`：自訂 `.wsz` 面板目錄

## 建置與 CLI 檢測工具

```bash
# 格式化、靜態分析與全工作區測試
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 執行 8-crate 端對端自我檢測
cargo run -p pilimao-app -- self-test

# 檢視 Clean-Room 預設面板或外部 .wsz 面板
cargo run -p pilimao-app -- inspect-skin default
cargo run -p pilimao-app -- inspect-skin path/to/classic.wsz

# 檢視 Winamp 二進位 .eqf 等化器預設
cargo run -p pilimao-app -- inspect-eqf path/to/preset.eqf

# 檢視音訊檔案標籤、舊版亂碼還原結果、ReplayGain 2.0 與 Bit-Perfect 資格
cargo run -p pilimao-app -- inspect-track path/to/song.mp3 --encoding big5
```

## Windows MSVC Portable ZIP 建置（CI）

GitHub Actions 工作流程 [`.github/workflows/ci.yml`](.github/workflows/ci.yml) 會自動：
1. 在 Linux 上執行 `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、跨目標 `x86_64-pc-windows-msvc` Clippy 型別檢查、`cargo test` 與 `self-test`。
2. 執行 `cargo-deny` 檢查授權與相依性安全。
3. 在 `windows-latest` 上以 MSVC 編譯最佳化版本並封裝包含 `pilimao.exe`、`portable.ini`、`LICENSE`、`README.md` 與 `skins/` 的 `pilimao-windows-x86_64-portable.zip` Artifact。

## 授權條款

本專案採 [MIT License](LICENSE) 授權。預設面板為 100% Clean-Room 程式繪製，不含任何受版權保護的第三方點陣圖素材。
