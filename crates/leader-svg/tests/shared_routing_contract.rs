use leader_core::{build_topology, orthogonal_route_for_link, MatchTrace};
use leader_svg::{render, RenderConfig};

fn svg_path(route: [[f32; 2]; 4]) -> String {
    format!(
        "M{:.1} {:.1}H{:.1}V{:.1}H{:.1}",
        route[0][0], route[0][1], route[1][0], route[2][1], route[3][0]
    )
}

#[test]
fn readme_wire_geometry_matches_core_router() {
    let topology = build_topology();
    let trace = MatchTrace::new("shared-routing-contract".to_owned(), 0);
    let svg = render(&topology, &trace, RenderConfig::default());

    for link in &topology.links {
        let route = orthogonal_route_for_link(&topology, link)
            .unwrap_or_else(|| panic!("closed topology must route link {}", link.id));
        let expected = format!("d=\"{}\"", svg_path(route));
        assert!(
            svg.contains(&expected),
            "README renderer route diverged from leader-core for link {}: {}",
            link.id,
            expected
        );
    }
}
