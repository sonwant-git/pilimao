//! Classic Winamp 2.x keyboard shortcut mapping.

/// Keyboard modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// High-level player action triggered by a Winamp keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    PreviousTrack,
    Play,
    PauseToggle,
    Stop,
    NextTrack,
    SeekBackward5s,
    SeekForward5s,
    VolumeUp,
    VolumeDown,
    ToggleShuffle,
    CycleRepeat,
    OpenFiles,
    OpenJumpToFile,
    EnqueueSelected,
    ToggleEqualizerWindow,
    TogglePlaylistWindow,
    CycleScale,
    ToggleWindowShade,
    CloseJumpOverlay,
}

/// Maps a Win32 Virtual-Key code + modifiers to a classic Winamp 2.x [`ShortcutAction`].
#[must_use]
pub fn map_virtual_key(vk: u32, mods: Modifiers) -> Option<ShortcutAction> {
    const VK_ESCAPE: u32 = 0x1B;
    const VK_LEFT: u32 = 0x25;
    const VK_UP: u32 = 0x26;
    const VK_RIGHT: u32 = 0x27;
    const VK_DOWN: u32 = 0x28;

    if vk == VK_ESCAPE {
        return Some(ShortcutAction::CloseJumpOverlay);
    }

    if mods.ctrl && !mods.alt {
        return match vk {
            0x44 => Some(ShortcutAction::CycleScale), // Ctrl+D (1x/2x/3x)
            0x57 => Some(ShortcutAction::ToggleWindowShade), // Ctrl+W
            _ => None,
        };
    }

    if mods.alt && !mods.ctrl {
        return match vk {
            0x47 => Some(ShortcutAction::ToggleEqualizerWindow), // Alt+G
            0x45 => Some(ShortcutAction::TogglePlaylistWindow),  // Alt+E
            _ => None,
        };
    }

    if !mods.ctrl && !mods.alt {
        return match vk {
            0x5A => Some(ShortcutAction::PreviousTrack),   // Z
            0x58 => Some(ShortcutAction::Play),            // X
            0x43 => Some(ShortcutAction::PauseToggle),     // C
            0x56 => Some(ShortcutAction::Stop),            // V
            0x42 => Some(ShortcutAction::NextTrack),       // B
            0x4C => Some(ShortcutAction::OpenFiles),       // L (Open File(s))
            0x53 => Some(ShortcutAction::ToggleShuffle),   // S
            0x52 => Some(ShortcutAction::CycleRepeat),     // R
            0x4A => Some(ShortcutAction::OpenJumpToFile),  // J
            0x51 => Some(ShortcutAction::EnqueueSelected), // Q
            VK_LEFT => Some(ShortcutAction::SeekBackward5s),
            VK_RIGHT => Some(ShortcutAction::SeekForward5s),
            VK_UP => Some(ShortcutAction::VolumeUp),
            VK_DOWN => Some(ShortcutAction::VolumeDown),
            _ => None,
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_classic_winamp_zxcvb_and_modifier_shortcuts() {
        let none = Modifiers::default();
        assert_eq!(
            map_virtual_key(b'Z' as u32, none),
            Some(ShortcutAction::PreviousTrack)
        );
        assert_eq!(
            map_virtual_key(b'X' as u32, none),
            Some(ShortcutAction::Play)
        );
        assert_eq!(
            map_virtual_key(b'C' as u32, none),
            Some(ShortcutAction::PauseToggle)
        );
        assert_eq!(
            map_virtual_key(b'V' as u32, none),
            Some(ShortcutAction::Stop)
        );
        assert_eq!(
            map_virtual_key(b'B' as u32, none),
            Some(ShortcutAction::NextTrack)
        );
        assert_eq!(
            map_virtual_key(b'L' as u32, none),
            Some(ShortcutAction::OpenFiles)
        );
        assert_eq!(
            map_virtual_key(b'J' as u32, none),
            Some(ShortcutAction::OpenJumpToFile)
        );

        let ctrl = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(
            map_virtual_key(b'D' as u32, ctrl),
            Some(ShortcutAction::CycleScale)
        );

        let alt = Modifiers {
            alt: true,
            ..Default::default()
        };
        assert_eq!(
            map_virtual_key(b'G' as u32, alt),
            Some(ShortcutAction::ToggleEqualizerWindow)
        );
        assert_eq!(
            map_virtual_key(b'E' as u32, alt),
            Some(ShortcutAction::TogglePlaylistWindow)
        );
    }
}
