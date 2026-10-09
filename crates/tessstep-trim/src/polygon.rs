use crate::{Containment, Error, ErrorKind, Remaining as Budget, err};
fn finite(x: f64) -> Result<f64, Error> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(err(ErrorKind::Geometry))
    }
}
pub fn distance(a: [f64; 2], b: [f64; 2]) -> Result<f64, Error> {
    finite((a[0] - b[0]).hypot(a[1] - b[1]))
}
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Result<f64, Error> {
    finite((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))
}
pub fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> Result<f64, Error> {
    let length = distance(a, b)?;
    if length == 0. {
        return distance(p, a);
    }
    let d = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
    let t = finite((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1])?.clamp(0., length);
    distance(p, [a[0] + t * d[0], a[1] + t * d[1]])
}
fn touches(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2], eps: f64) -> Result<bool, Error> {
    if segment_distance(a, c, d)? <= eps
        || segment_distance(b, c, d)? <= eps
        || segment_distance(c, a, b)? <= eps
        || segment_distance(d, a, b)? <= eps
    {
        return Ok(true);
    }
    let x = orient(a, b, c)?;
    let y = orient(a, b, d)?;
    let z = orient(c, d, a)?;
    let w = orient(c, d, b)?;
    Ok(x.signum() != y.signum() && z.signum() != w.signum())
}
pub fn classify(poly: &[[f64; 2]], p: [f64; 2], eps: f64) -> Result<Containment, Error> {
    finite(p[0])?;
    finite(p[1])?;
    let mut inside = false;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if segment_distance(p, a, b)? <= eps {
            return Ok(Containment::Boundary);
        }
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let cross = orient(a, b, p)?;
            if (cross > 0.) == (b[1] > a[1]) {
                inside = !inside;
            }
        }
    }
    Ok(if inside {
        Containment::Inside
    } else {
        Containment::Outside
    })
}

/// A failed polygon check located at the segments involved (segment `i` runs from
/// point `i` to the next); loop-level failures name no segment.
#[derive(Debug)]
pub struct Defect {
    pub error: Error,
    pub at: Option<usize>,
    pub other: Option<usize>,
}
impl From<Error> for Defect {
    fn from(error: Error) -> Self {
        Self {
            error,
            at: None,
            other: None,
        }
    }
}
fn defect(kind: ErrorKind, at: Option<usize>, other: Option<usize>) -> Defect {
    Defect {
        error: err(kind),
        at,
        other,
    }
}

/// Axis-aligned box of the points `poly[i]` and `poly[i + 1]`, or of a whole polygon.
#[derive(Clone, Copy)]
pub struct Bounds {
    lo: [f64; 2],
    hi: [f64; 2],
}
impl Bounds {
    pub fn of(points: impl IntoIterator<Item = [f64; 2]>) -> Self {
        let mut b = Self {
            lo: [f64::INFINITY; 2],
            hi: [f64::NEG_INFINITY; 2],
        };
        for p in points {
            b.lo = [b.lo[0].min(p[0]), b.lo[1].min(p[1])];
            b.hi = [b.hi[0].max(p[0]), b.hi[1].max(p[1])];
        }
        b
    }
    fn overlaps(&self, other: &Self, k: usize, eps: f64) -> bool {
        self.lo[k] <= other.hi[k] + eps && other.lo[k] <= self.hi[k] + eps
    }
}
fn segments(poly: &[[f64; 2]]) -> impl Iterator<Item = Bounds> + '_ {
    (0..poly.len()).map(|i| Bounds::of([poly[i], poly[(i + 1) % poly.len()]]))
}

/// Visit every pair of boxes that overlap within `eps`, sweeping along the axis of
/// the larger extent: boxes are ordered by their low end there and each is compared
/// only with the earlier boxes still reaching it. Every such comparison charges one
/// work unit, so the budget bounds the worst case (all boxes overlapping, about half
/// of the n² units the all-pairs checks charged) and boundaries that are spread out
/// cost far less. `visit` returns whether to stop; pairs are passed in increasing
/// index order.
pub fn sweep(
    boxes: &[Bounds],
    eps: f64,
    budget: &mut Budget,
    mut visit: impl FnMut(usize, usize, &mut Budget) -> Result<bool, Defect>,
) -> Result<(), Defect> {
    let all = Bounds::of(boxes.iter().flat_map(|b| [b.lo, b.hi]));
    let k = usize::from(all.hi[1] - all.lo[1] > all.hi[0] - all.lo[0]);
    let j = 1 - k;
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| boxes[a].lo[k].total_cmp(&boxes[b].lo[k]).then(a.cmp(&b)));
    let mut active: Vec<usize> = Vec::new();
    for &i in &order {
        let reach = boxes[i].lo[k] - eps;
        budget.work(active.len())?;
        active.retain(|&a| boxes[a].hi[k] >= reach);
        for &a in &active {
            if boxes[a].overlaps(&boxes[i], j, eps) && visit(a.min(i), a.max(i), budget)? {
                return Ok(());
            }
        }
        active.push(i);
    }
    Ok(())
}

/// Whether a conflict between segments `i < j` is a wobble below the tolerance rather
/// than a defect of the boundary: for adjacent segments, the shorter of the two is at
/// most `noise` long (the loop doubles back by no more than that); otherwise the part
/// of the loop strictly between them (the shorter way round) stays within a box whose
/// diagonal is at most `noise`.
fn local(
    poly: &[[f64; 2]],
    i: usize,
    j: usize,
    noise: f64,
    budget: &mut Budget,
) -> Result<bool, Error> {
    let n = poly.len();
    if j == i + 1 {
        // `j` may be `n`: the last segment and the first.
        let (a, b, c) = (poly[i % n], poly[j % n], poly[(j + 1) % n]);
        return Ok(distance(a, b)?.min(distance(b, c)?) <= noise);
    }
    let forward = j - i;
    let backward = n - (j - i);
    let (start, count) = if forward <= backward {
        (i + 1, forward)
    } else {
        (j + 1, backward)
    };
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for step in 0..count.min(n) {
        budget.work(1)?;
        let p = poly[(start + step) % n];
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        if (hi[0] - lo[0]).hypot(hi[1] - lo[1]) > noise {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Self-validation of one closed polygon; returns its signed area. Points closer than
/// `eps` touch. With a positive `noise`, conflicts confined to a part of the loop
/// within `noise` (the model tolerance) are ignored; with zero every conflict counts.
pub fn validate(
    poly: &[[f64; 2]],
    eps: f64,
    noise: f64,
    budget: &mut Budget,
) -> Result<f64, Defect> {
    let n = poly.len();
    if n < 3 {
        return Err(defect(ErrorKind::DegenerateLoop, None, None));
    }
    budget.work(n)?;
    let tolerant = noise > 0.;
    let mut area = 0.;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let c = poly[(i + 2) % n];
        area = finite(area + orient(poly[0], a, b)? * 0.5)?;
        if distance(a, b)? <= eps {
            if tolerant {
                continue;
            }
            return Err(defect(ErrorKind::DegenerateLoop, Some(i), None));
        }
        // Adjacent segments may share just their endpoint, not backtrack.
        if (segment_distance(a, b, c)? <= eps || segment_distance(c, a, b)? <= eps)
            && !(tolerant && local(poly, i, i + 1, noise, budget)?)
        {
            return Err(defect(
                ErrorKind::SelfIntersection,
                Some(i),
                Some((i + 1) % n),
            ));
        }
    }
    let boxes: Vec<Bounds> = segments(poly).collect();
    let mut found = None;
    sweep(&boxes, eps, budget, |i, j, budget| {
        if j == i + 1 || (i == 0 && j == n - 1) {
            return Ok(false);
        }
        if touches(poly[i], poly[(i + 1) % n], poly[j], poly[(j + 1) % n], eps)?
            && !(tolerant && local(poly, i, j, noise, budget)?)
        {
            found = Some((i, j));
            return Ok(true);
        }
        Ok(false)
    })?;
    if let Some((i, j)) = found {
        return Err(defect(ErrorKind::SelfIntersection, Some(i), Some(j)));
    }
    if area.abs() <= eps * eps {
        return Err(defect(ErrorKind::DegenerateLoop, None, None));
    }
    Ok(area)
}
/// Two loops share no point within `eps`. A conflict names a segment of each.
pub fn disjoint(
    a: &[[f64; 2]],
    b: &[[f64; 2]],
    eps: f64,
    budget: &mut Budget,
) -> Result<(), Defect> {
    budget.work(1)?;
    let (ba, bb) = (Bounds::of(a.iter().copied()), Bounds::of(b.iter().copied()));
    if !(ba.overlaps(&bb, 0, eps) && ba.overlaps(&bb, 1, eps)) {
        return Ok(());
    }
    budget.work(a.len().saturating_add(b.len()))?;
    let boxes: Vec<Bounds> = segments(a).chain(segments(b)).collect();
    let m = a.len();
    let mut found = None;
    sweep(&boxes, eps, budget, |i, j, _| {
        // Only pairs with one segment from each loop.
        if i >= m || j < m {
            return Ok(false);
        }
        let k = j - m;
        if touches(a[i], a[(i + 1) % m], b[k], b[(k + 1) % b.len()], eps)? {
            found = Some((i, k));
            return Ok(true);
        }
        Ok(false)
    })?;
    match found {
        Some((i, k)) => Err(defect(ErrorKind::IntersectingLoops, Some(i), Some(k))),
        None => Ok(()),
    }
}
