use crate::{Budget, Containment, Error, ErrorKind, err};
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
pub fn validate(poly: &[[f64; 2]], eps: f64, budget: &mut Budget) -> Result<f64, Error> {
    let n = poly.len();
    if n < 3 {
        return Err(err(ErrorKind::DegenerateLoop));
    }
    budget.work(n.checked_mul(n).ok_or(err(ErrorKind::Limit))?)?;
    let mut area = 0.;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let c = poly[(i + 2) % n];
        if distance(a, b)? <= eps {
            return Err(err(ErrorKind::DegenerateLoop));
        }
        // Adjacent segments may share just their endpoint, not backtrack.
        if segment_distance(a, b, c)? <= eps || segment_distance(c, a, b)? <= eps {
            return Err(err(ErrorKind::SelfIntersection));
        }
        area = finite(area + orient(poly[0], a, b)? * 0.5)?;
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if touches(a, b, poly[j], poly[(j + 1) % n], eps)? {
                return Err(err(ErrorKind::SelfIntersection));
            }
        }
    }
    if area.abs() <= eps * eps {
        return Err(err(ErrorKind::DegenerateLoop));
    }
    Ok(area)
}
pub fn disjoint(
    a: &[[f64; 2]],
    b: &[[f64; 2]],
    eps: f64,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.work(a.len().checked_mul(b.len()).ok_or(err(ErrorKind::Limit))?)?;
    for i in 0..a.len() {
        for j in 0..b.len() {
            if touches(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()], eps)? {
                return Err(err(ErrorKind::IntersectingLoops));
            }
        }
    }
    Ok(())
}
