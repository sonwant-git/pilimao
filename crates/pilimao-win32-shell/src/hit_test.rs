//! Scale-independent hit-testing for Main, Equalizer, and Playlist interactive controls.

use pilimao_dsp::{EQ_MAX_DB, EQ_MIN_DB};
use pilimao_skin_wsz::SHADE_HEIGHT;

/// Interactive control regions inside the Main window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MainHit {
    TitleBar,
    Minimize,
    WindowShade,
    Close,
    VisToggle,
    VolumeSlider(f64),
    BalanceSlider(f64),
    SeekBar(f64),
    ToggleEq,
    TogglePl,
    Prev,
    Play,
    Pause,
    Stop,
    Next,
    Eject,
    ToggleShuffle,
    CycleRepeat,
    None,
}

/// Interactive control regions inside the Equalizer window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EqHit {
    TitleBar,
    WindowShade,
    Close,
    ToggleOn,
    PreampSlider(f64),
    BandSlider { band: usize, gain_db: f64 },
    None,
}

/// Interactive control regions inside the Playlist Editor window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlHit {
    TitleBar,
    WindowShade,
    Close,
    TrackRow(usize),
    ResizeCorner,
    None,
}

/// Hit-tests the Main window at 1x logical coordinates `(lx, ly)`.
#[must_use]
pub fn hit_test_main(lx: i32, ly: i32, shaded: bool) -> MainHit {
    if ly < SHADE_HEIGHT {
        if (244..253).contains(&lx) {
            return MainHit::Minimize;
        }
        if (254..263).contains(&lx) {
            return MainHit::WindowShade;
        }
        if (264..273).contains(&lx) {
            return MainHit::Close;
        }
        return MainHit::TitleBar;
    }

    if shaded {
        return MainHit::None;
    }

    if (24..100).contains(&lx) && (43..59).contains(&ly) {
        return MainHit::VisToggle;
    }
    if (107..175).contains(&lx) && (57..70).contains(&ly) {
        let v = ((lx - 107) as f64 / 67.0).clamp(0.0, 1.0);
        return MainHit::VolumeSlider(v);
    }
    if (177..215).contains(&lx) && (57..70).contains(&ly) {
        let mut b = ((lx - 177) as f64 / 37.0) * 2.0 - 1.0;
        if b.abs() < 0.08 {
            b = 0.0; // Center detent like Winamp
        }
        return MainHit::BalanceSlider(b.clamp(-1.0, 1.0));
    }
    if (219..242).contains(&lx) && (58..70).contains(&ly) {
        return MainHit::ToggleEq;
    }
    if (242..265).contains(&lx) && (58..70).contains(&ly) {
        return MainHit::TogglePl;
    }
    if (16..264).contains(&lx) && (72..82).contains(&ly) {
        let ratio = ((lx - 16) as f64 / 247.0).clamp(0.0, 1.0);
        return MainHit::SeekBar(ratio);
    }
    if (88..106).contains(&ly) {
        if (16..39).contains(&lx) {
            return MainHit::Prev;
        }
        if (39..62).contains(&lx) {
            return MainHit::Play;
        }
        if (62..85).contains(&lx) {
            return MainHit::Pause;
        }
        if (85..108).contains(&lx) {
            return MainHit::Stop;
        }
        if (108..130).contains(&lx) {
            return MainHit::Next;
        }
        if (136..158).contains(&lx) {
            return MainHit::Eject;
        }
    }
    if (89..104).contains(&ly) {
        if (164..211).contains(&lx) {
            return MainHit::ToggleShuffle;
        }
        if (210..238).contains(&lx) {
            return MainHit::CycleRepeat;
        }
    }

    MainHit::None
}

/// Hit-tests the Equalizer window at 1x logical coordinates `(lx, ly)`.
#[must_use]
pub fn hit_test_eq(lx: i32, ly: i32, shaded: bool) -> EqHit {
    if ly < SHADE_HEIGHT {
        if (254..263).contains(&lx) {
            return EqHit::WindowShade;
        }
        if (264..273).contains(&lx) {
            return EqHit::Close;
        }
        return EqHit::TitleBar;
    }

    if shaded {
        return EqHit::None;
    }

    if (14..39).contains(&lx) && (18..30).contains(&ly) {
        return EqHit::ToggleOn;
    }

    if (38..101).contains(&ly) {
        let norm = 1.0 - ((ly - 38) as f64 / 62.0).clamp(0.0, 1.0);
        let mut gain_db = EQ_MIN_DB + norm * (EQ_MAX_DB - EQ_MIN_DB);
        if gain_db.abs() < 0.6 {
            gain_db = 0.0; // Center 0 dB snap detent
        }
        if (21..35).contains(&lx) {
            return EqHit::PreampSlider(gain_db);
        }
        for band in 0..10usize {
            let bx = 78 + (band as i32) * 18;
            if (bx..bx + 14).contains(&lx) {
                return EqHit::BandSlider { band, gain_db };
            }
        }
    }

    EqHit::None
}

/// Hit-tests the Playlist window at 1x logical coordinates `(lx, ly)`.
#[must_use]
pub fn hit_test_playlist(
    lx: i32,
    ly: i32,
    logical_w: i32,
    logical_h: i32,
    shaded: bool,
    scroll_offset: usize,
) -> PlHit {
    if ly < SHADE_HEIGHT {
        if lx >= logical_w - 22 && lx < logical_w - 12 {
            return PlHit::WindowShade;
        }
        if lx >= logical_w - 12 {
            return PlHit::Close;
        }
        return PlHit::TitleBar;
    }

    if shaded {
        return PlHit::None;
    }

    if lx >= logical_w - 20 && ly >= logical_h - 20 {
        return PlHit::ResizeCorner;
    }

    let list_bottom = logical_h - 38;
    if ly >= 22 && ly < list_bottom && lx >= 6 && lx < logical_w - 12 {
        let row = ((ly - 22) / 9) as usize;
        return PlHit::TrackRow(scroll_offset + row);
    }

    PlHit::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_tests_transport_and_eq_sliders() {
        assert_eq!(hit_test_main(45, 95, false), MainHit::Play);
        assert_eq!(hit_test_main(70, 95, false), MainHit::Pause);
        assert_eq!(hit_test_main(268, 5, false), MainHit::Close);
        assert_eq!(hit_test_eq(20, 22, false), EqHit::ToggleOn);
        match hit_test_eq(82, 38, false) {
            EqHit::BandSlider { band: 0, gain_db } => assert!((gain_db - 12.0).abs() < 0.1),
            other => panic!("unexpected eq hit: {other:?}"),
        }
    }
}
