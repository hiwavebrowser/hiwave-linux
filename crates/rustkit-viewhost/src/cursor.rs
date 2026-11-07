//! The CSS `cursor` keyword the page reports (`PageEvent::Cursor`, CH2.E4)
//! as the AppKit cursor the content view shows. Plain data, so the mapping
//! and its tests build on every OS; `macos.rs` sets the cursor.

/// The `NSCursor` class method that returns the cursor for a used CSS
/// `cursor` keyword. AppKit has no public cursor for some keywords
/// (`help`, `wait`, `progress`, `cell`, the diagonal resizes, `zoom-in`,
/// `zoom-out`, `none`): they show the arrow.
pub fn ns_cursor_selector(css: &str) -> &'static str {
    match css {
        "pointer" => "pointingHandCursor",
        "text" => "IBeamCursor",
        "vertical-text" => "IBeamCursorForVerticalLayout",
        "ew-resize" | "col-resize" => "resizeLeftRightCursor",
        "ns-resize" | "row-resize" => "resizeUpDownCursor",
        "e-resize" => "resizeRightCursor",
        "w-resize" => "resizeLeftCursor",
        "n-resize" => "resizeUpCursor",
        "s-resize" => "resizeDownCursor",
        "not-allowed" | "no-drop" => "operationNotAllowedCursor",
        // AppKit has no public four-way move cursor.
        "grab" | "move" | "all-scroll" => "openHandCursor",
        "grabbing" => "closedHandCursor",
        "crosshair" => "crosshairCursor",
        "copy" => "dragCopyCursor",
        "alias" => "dragLinkCursor",
        "context-menu" => "contextualMenuCursor",
        _ => "arrowCursor",
    }
}

/// The X11 font-cursor glyph (`cursorfont.h`) for a used CSS `cursor` keyword. Plain data, like the AppKit mapping.
/// Keywords X's cursor font has no good glyph for show the arrow.
pub fn x11_cursor_shape(css: &str) -> u32 {
    const LEFT_PTR: u32 = 68;
    match css {
        "pointer" => 60,                                 // XC_hand2
        "text" | "vertical-text" => 152,                 // XC_xterm
        "ew-resize" | "col-resize" => 108,               // XC_sb_h_double_arrow
        "ns-resize" | "row-resize" => 116,               // XC_sb_v_double_arrow
        "e-resize" => 96,                                // XC_right_side
        "w-resize" => 70,                                // XC_left_side
        "n-resize" => 138,                               // XC_top_side
        "s-resize" => 16,                                // XC_bottom_side
        "not-allowed" | "no-drop" => 24,                 // XC_circle
        "grab" => 58,                                    // XC_hand1
        "grabbing" | "move" | "all-scroll" => 52,        // XC_fleur
        "crosshair" => 34,                               // XC_crosshair
        "wait" | "progress" => 150,                      // XC_watch
        "help" => 92,                                    // XC_question_arrow
        "copy" | "alias" | "cell" => 90,                 // XC_plus
        _ => LEFT_PTR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chrome_and_page_keywords_map_to_appkit_cursors() {
        for (css, ns) in [
            ("default", "arrowCursor"),
            ("auto", "arrowCursor"),
            ("pointer", "pointingHandCursor"),
            ("text", "IBeamCursor"),
            ("vertical-text", "IBeamCursorForVerticalLayout"),
            ("ew-resize", "resizeLeftRightCursor"),
            ("col-resize", "resizeLeftRightCursor"),
            ("ns-resize", "resizeUpDownCursor"),
            ("row-resize", "resizeUpDownCursor"),
            ("e-resize", "resizeRightCursor"),
            ("w-resize", "resizeLeftCursor"),
            ("n-resize", "resizeUpCursor"),
            ("s-resize", "resizeDownCursor"),
            ("not-allowed", "operationNotAllowedCursor"),
            ("no-drop", "operationNotAllowedCursor"),
            ("grab", "openHandCursor"),
            ("grabbing", "closedHandCursor"),
            ("move", "openHandCursor"),
            ("all-scroll", "openHandCursor"),
            ("crosshair", "crosshairCursor"),
            ("copy", "dragCopyCursor"),
            ("alias", "dragLinkCursor"),
            ("context-menu", "contextualMenuCursor"),
            ("help", "arrowCursor"),
            ("wait", "arrowCursor"),
            ("nonsense", "arrowCursor"),
        ] {
            assert_eq!(ns_cursor_selector(css), ns, "{css}");
        }
    }

    #[test]
    fn css_cursors_map_to_x11_font_cursors() {
        for (css, shape) in [
            ("default", 68), ("auto", 68), ("pointer", 60), ("text", 152), ("ew-resize", 108), ("row-resize", 116),
            ("not-allowed", 24), ("grab", 58), ("move", 52), ("crosshair", 34), ("wait", 150), ("help", 92), ("copy", 90),
            ("zoom-in", 68),
        ] {
            assert_eq!(x11_cursor_shape(css), shape, "{css}");
        }
    }
}
