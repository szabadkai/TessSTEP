//! FACE_OUTER_BOUND labels that the loop geometry contradicts (see
//! docs/CURVED_IMPORT.md#face-charts).
//!
//! Face loops are planned with the face to their left in the surface chart, so an
//! outer bound winds counterclockwise and a hole clockwise. A FACE_OUTER_BOUND on a
//! clockwise loop beside exactly one counterclockwise loop that encloses every other
//! loop is a label on a hole: the bound senses and their nesting agree on the other
//! loop as the outer bound. No loop, sense or coordinate changes; only which loop
//! bounds the face from outside.
use super::*;

/// The loop that bounds the face from outside when the FACE_OUTER_BOUND `declared`
/// contradicts the geometry of `loops` (all edge loops of the face, `declared`
/// among them), or `None` when the declared bound is consistent or no other loop is
/// unambiguously the outer one.
pub(super) fn contradicted(
    declared: usize,
    loops: &[usize],
    charts: &BTreeMap<usize, LoopChart>,
) -> Option<usize> {
    if loops.len() < 2 || signed_area(&charts[&declared].polygon) >= 0. {
        return None;
    }
    let mut ccw = loops
        .iter()
        .copied()
        .filter(|i| signed_area(&charts[i].polygon) > 0.);
    let (Some(outer), None) = (ccw.next(), ccw.next()) else {
        return None;
    };
    let boundary = &charts[&outer].polygon;
    loops
        .iter()
        .filter(|&&i| i != outer)
        .all(|i| charts[i].polygon.iter().all(|&p| winds(boundary, p)))
        .then_some(outer)
}

/// Whether the closed polygon winds around `p` (nonzero winding number; a point on
/// the boundary may count either way).
fn winds(polygon: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = polygon.len();
    let mut winding = 0_i64;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        let side = (b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1]);
        if a[1] <= p[1] && b[1] > p[1] && side > 0. {
            winding += 1;
        } else if a[1] > p[1] && b[1] <= p[1] && side < 0. {
            winding -= 1;
        }
    }
    winding != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(polygon: &[[f64; 2]]) -> LoopChart {
        LoopChart {
            starts: Vec::new(),
            winding: [0, 0],
            polygon: polygon.to_vec(),
        }
    }
    const SQUARE: [[f64; 2]; 4] = [[0., 0.], [4., 0.], [4., 4.], [0., 4.]];
    const HOLE: [[f64; 2]; 4] = [[1., 1.], [1., 2.], [2., 2.], [2., 1.]];
    const FAR: [[f64; 2]; 4] = [[6., 1.], [6., 2.], [7., 2.], [7., 1.]];

    fn reversed(p: &[[f64; 2]]) -> Vec<[f64; 2]> {
        p.iter().rev().copied().collect()
    }

    #[test]
    fn a_label_on_an_enclosed_clockwise_loop_names_the_enclosing_loop() {
        let charts = BTreeMap::from([(0, chart(&HOLE)), (1, chart(&SQUARE))]);
        assert_eq!(contradicted(0, &[0, 1], &charts), Some(1));
        // A consistent label is kept.
        assert_eq!(contradicted(1, &[0, 1], &charts), None);
    }

    #[test]
    fn reversed_or_unenclosed_loops_keep_the_declared_bound() {
        // Every loop reversed: the counterclockwise loop does not enclose the label.
        let charts = BTreeMap::from([(0, chart(&reversed(&SQUARE))), (1, chart(&reversed(&HOLE)))]);
        assert_eq!(contradicted(0, &[0, 1], &charts), None);
        // A clockwise label beside a counterclockwise loop that encloses only part
        // of the face's loops.
        let charts = BTreeMap::from([(0, chart(&FAR)), (1, chart(&SQUARE)), (2, chart(&HOLE))]);
        assert_eq!(contradicted(0, &[0, 1, 2], &charts), None);
        // Two counterclockwise candidates are ambiguous.
        let charts = BTreeMap::from([
            (0, chart(&HOLE)),
            (1, chart(&SQUARE)),
            (2, chart(&reversed(&FAR))),
        ]);
        assert_eq!(contradicted(0, &[0, 1, 2], &charts), None);
        // A single clockwise loop has no other reading.
        let charts = BTreeMap::from([(0, chart(&HOLE))]);
        assert_eq!(contradicted(0, &[0], &charts), None);
    }
}
