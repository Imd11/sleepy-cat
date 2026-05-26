use crate::platform::CandidateInput;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayOffset {
    pub x: f64,
    pub y: f64,
}

pub fn prompt_button_position(
    input: &CandidateInput,
    window: &CandidateInput,
    button_width: f64,
    button_height: f64,
    offset: Option<OverlayOffset>,
) -> OverlayPoint {
    let base_x = input.x + input.width - button_width - 12.0;
    let base_y = input.y - button_height - 8.0;
    let offset = offset.unwrap_or(OverlayOffset { x: 0.0, y: 0.0 });

    let min_x = window.x + 12.0;
    let max_x = window.x + window.width - button_width - 12.0;
    let min_y = window.y + 12.0;
    let max_y = window.y + window.height - button_height - 12.0;

    OverlayPoint {
        x: (base_x + offset.x).clamp(min_x, max_x),
        y: (base_y + offset.y).clamp(min_y, max_y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::CandidateInput;

    #[test]
    fn positions_button_inside_input_panel_near_top_right() {
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 1200.0,
            height: 900.0,
        };
        let input = CandidateInput {
            x: 300.0,
            y: 748.0,
            width: 600.0,
            height: 128.0,
        };

        let pos = prompt_button_position(&input, &window, 112.0, 40.0, None);

        assert_eq!(pos.x, 776.0);
        assert_eq!(pos.y, 700.0);
    }

    #[test]
    fn clamps_drag_offset_inside_target_window() {
        let window = CandidateInput {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 500.0,
        };
        let input = CandidateInput {
            x: 100.0,
            y: 360.0,
            width: 400.0,
            height: 100.0,
        };
        let offset = OverlayOffset { x: 9999.0, y: 9999.0 };

        let pos = prompt_button_position(&input, &window, 112.0, 40.0, Some(offset));

        assert_eq!(pos.x, 476.0);
        assert_eq!(pos.y, 448.0);
    }
}