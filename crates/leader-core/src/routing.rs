use crate::topology::{Link, Rect, Topology};

pub type OrthogonalRoute = [[f32; 2]; 4];

/// Produces the canonical four-point orthogonal route between two physical
/// node bounds. This geometry is renderer-independent and belongs to the core.
#[must_use]
pub fn orthogonal_route_between(from: Rect, to: Rect) -> OrthogonalRoute {
    let from_center = [from.x + from.w * 0.5, from.y + from.h * 0.5];
    let to_center = [to.x + to.w * 0.5, to.y + to.h * 0.5];
    let travels_right = to_center[0] >= from_center[0];
    let start_x = if travels_right { from.x + from.w } else { from.x };
    let end_x = if travels_right { to.x } else { to.x + to.w };
    let middle_x = (start_x + end_x) * 0.5;
    [
        [start_x, from_center[1]],
        [middle_x, from_center[1]],
        [middle_x, to_center[1]],
        [end_x, to_center[1]],
    ]
}

#[must_use]
pub fn orthogonal_route_for_link(topology: &Topology, link: &Link) -> Option<OrthogonalRoute> {
    let from = topology.node(&link.from)?;
    let to = topology.node(&link.to)?;
    Some(orthogonal_route_between(from.bounds, to.bounds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_terminates_on_facing_horizontal_edges() {
        let from = Rect::new(10.0, 20.0, 30.0, 40.0);
        let to = Rect::new(100.0, 80.0, 20.0, 20.0);
        let route = orthogonal_route_between(from, to);
        assert_eq!(route[0], [40.0, 40.0]);
        assert_eq!(route[3], [100.0, 90.0]);
        assert_eq!(route[0][1], route[1][1]);
        assert_eq!(route[1][0], route[2][0]);
        assert_eq!(route[2][1], route[3][1]);
    }

    #[test]
    fn route_handles_right_to_left_links_symmetrically() {
        let from = Rect::new(100.0, 80.0, 20.0, 20.0);
        let to = Rect::new(10.0, 20.0, 30.0, 40.0);
        let route = orthogonal_route_between(from, to);
        assert_eq!(route[0], [100.0, 90.0]);
        assert_eq!(route[3], [40.0, 40.0]);
    }
}
