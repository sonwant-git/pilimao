//! `pilimao-skin-wsz`: Lenient Winamp 2.x `.wsz` skin loader, `region.txt`/`viscolor.txt`/`pledit.txt`
//! parsers, 100% Clean-Room default skin, and 3-window magnetic docking / BFS / Windowshade engine.

pub mod cleanroom;
pub mod docking;
pub mod loader;
pub mod pledit;
pub mod region;
pub mod viscolor;

pub use cleanroom::{
    SkinReport, SkinSprites, SpriteImage, WszSkin, draw_bitmap_char_5x6, draw_micro_text_3x5,
    micro_glyph_3x5,
};
pub use docking::{
    DEFAULT_SNAP_DISTANCE, DockingEngine, EQ_HEIGHT, EQ_WIDTH, MAIN_HEIGHT, MAIN_WIDTH,
    PL_MIN_HEIGHT, PL_MIN_WIDTH, PL_STEP_HEIGHT, PL_STEP_WIDTH, SHADE_HEIGHT, WindowId, WindowRect,
    WindowState,
};
pub use loader::{SkinError, load_skin, load_skin_from_wsz_bytes, write_wsz_bytes};
pub use pledit::{PleditStyle, parse_pledit_txt};
pub use region::{Polygon, RegionData, parse_region_txt};
pub use viscolor::{Rgb, VisColors, parse_viscolor_txt};
