//! Supplied pcurves: the PCURVE records of SURFACE_CURVE and SEAM_CURVE edges,
//! decoded with the bundled pcurve profile (`corpus/geometry/pcurve.exp`) and kept in
//! their surface's STEP parameters. Faces map them into their chart and verify them
//! against the 3D curve before use; see `Builder::supplied_pcurve`.
use super::*;
use crate::pcurve_profile;

/// A 2D curve in its surface's STEP parameters, with its STEP curve parameter `t`:
/// a line `o + t d`, an ellipse `c + a cos(k t) x + b sin(k t) y` (`y` the left
/// normal of `x`, `k` radians per plane-angle unit) or a NURBS curve.
#[derive(Clone, Debug)]
pub(crate) enum Curve2 {
    Line {
        origin: [f64; 2],
        direction: [f64; 2],
    },
    Conic {
        centre: [f64; 2],
        x: [f64; 2],
        a: f64,
        b: f64,
    },
    Nurbs {
        degree: usize,
        knots: Vec<f64>,
        controls: Vec<[f64; 2]>,
        weights: Vec<f64>,
    },
}

/// A supplied pcurve and the surface entity it lies on.
#[derive(Clone, Debug)]
pub(crate) struct Supplied {
    pub surface: EntityId,
    pub curve: Curve2,
}

/// Decode the PCURVE records among `candidates` (links retained on surface curves).
/// Records outside the profile are left out and counted.
pub(crate) fn decode(
    document: &Document,
    candidates: &[EntityId],
    options: ImportOptions,
) -> Result<(BTreeMap<EntityId, Supplied>, usize), Error> {
    use pcurve_profile::schema_tessstep_pcurve as s;
    use tessstep_schema::EntityBinding;
    let roots: Vec<EntityId> = candidates
        .iter()
        .copied()
        .filter(|id| {
            document.entities().get(*id).is_some_and(|e| {
                matches!(e.kind.records(), [r] if r.name.as_ref().eq_ignore_ascii_case("PCURVE"))
            })
        })
        .collect();
    let mut found = BTreeMap::new();
    if roots.is_empty() {
        return Ok((found, 0));
    }
    let run = |roots: &[EntityId]| {
        decode::decode_reachable_profile_with_links(
            document,
            &pcurve_profile::SCHEMA_SET,
            "tessstep_pcurve",
            roots,
            &[],
            &[decode::LinkSlot {
                entity: s::Entity_PCURVE::DECLARATION,
                attribute: "BASIS_SURFACE",
            }],
            decode::Limits {
                max_work: options.max_work,
                ..decode::Limits::default()
            },
        )
    };
    let limit = |e: decode::Error| Error {
        kind: ErrorKind::ResourceLimit,
        stage: Stage::Profile,
        entity: e.entity,
        source: e.source,
        message: e.to_string(),
    };
    // Decode together; when that fails, decode each alone and leave out failures.
    let mut kept = roots.clone();
    let decoded = match run(&roots) {
        Ok(d) => d,
        Err(e) if e.kind == decode::ErrorKind::ResourceLimit => return Err(limit(e)),
        Err(_) => {
            kept.clear();
            for &root in &roots {
                match run(&[root]) {
                    Ok(_) => kept.push(root),
                    Err(e) if e.kind == decode::ErrorKind::ResourceLimit => return Err(limit(e)),
                    Err(_) => {}
                }
            }
            if kept.is_empty() {
                return Ok((found, roots.len()));
            }
            run(&kept).map_err(limit)?
        }
    };
    let mut unreadable = roots.len() - kept.len();
    for &root in &kept {
        match read(&decoded, root) {
            Some(supplied) => {
                found.insert(root, supplied);
            }
            None => unreadable += 1,
        }
    }
    Ok((found, unreadable))
}

fn read<'a>(decoded: &DecodedDocument<'a>, root: EntityId) -> Option<Supplied> {
    let view = |id: EntityId| decoded.get(id);
    let is = |v: &EntityView<'_>, name: &str| {
        v.types.iter().any(|id| {
            decoded
                .schemas()
                .declaration(*id)
                .is_some_and(|d| d.name.eq_ignore_ascii_case(name))
        })
    };
    fn attr<'a>(v: &EntityView<'a>, name: &str) -> Option<&'a StepValue> {
        v.attributes
            .iter()
            .find(|a| a.declaration.name.eq_ignore_ascii_case(name))
            .map(|a| a.value)
    }
    let reference = |value: &StepValue| match value.kind {
        ValueKind::Reference(id) => Some(id),
        _ => None,
    };
    fn number(value: &StepValue) -> Option<f64> {
        match &value.kind {
            ValueKind::Real(x) => Some(*x),
            ValueKind::Integer(x) => Some(*x as f64),
            ValueKind::Typed { value, .. } => number(value),
            _ => None,
        }
    }
    let list = |value: &StepValue| -> Option<Vec<f64>> {
        match &value.kind {
            ValueKind::Aggregate(values) => values.iter().map(number).collect(),
            _ => None,
        }
    };
    let point2 = |id: EntityId| -> Option<[f64; 2]> {
        let c = list(attr(view(id)?, "COORDINATES")?)?;
        (c.len() == 2).then(|| [c[0], c[1]])
    };
    let direction2 = |id: EntityId| -> Option<[f64; 2]> {
        let d = list(attr(view(id)?, "DIRECTION_RATIOS")?)?;
        let n = d.first()?.hypot(*d.get(1)?);
        (d.len() == 2 && n > 0. && n.is_finite()).then(|| [d[0] / n, d[1] / n])
    };
    let pcurve = view(root)?;
    let surface = reference(attr(pcurve, "BASIS_SURFACE")?)?;
    let representation = view(reference(attr(pcurve, "REFERENCE_TO_CURVE")?)?)?;
    let ValueKind::Aggregate(items) = &attr(representation, "ITEMS")?.kind else {
        return None;
    };
    // The representation holds exactly one curve.
    let curves: Vec<&EntityView<'_>> = items
        .iter()
        .filter_map(|i| view(reference(i)?))
        .filter(|v| is(v, "CURVE"))
        .collect();
    let [v] = curves.as_slice() else {
        return None;
    };
    let curve = if is(v, "LINE") {
        let origin = point2(reference(attr(v, "PNT")?)?)?;
        let vector = view(reference(attr(v, "DIR")?)?)?;
        let d = direction2(reference(attr(vector, "ORIENTATION")?)?)?;
        let m = number(attr(vector, "MAGNITUDE")?)?;
        if !(m > 0. && m.is_finite()) {
            return None;
        }
        Curve2::Line {
            origin,
            direction: [d[0] * m, d[1] * m],
        }
    } else if is(v, "CIRCLE") || is(v, "ELLIPSE") {
        let position = view(reference(attr(v, "POSITION")?)?)?;
        let centre = point2(reference(attr(position, "LOCATION")?)?)?;
        let x = match attr(position, "REF_DIRECTION")? {
            value if matches!(value.kind, ValueKind::Null) => [1., 0.],
            value => direction2(reference(value)?)?,
        };
        let (a, b) = if is(v, "CIRCLE") {
            let r = number(attr(v, "RADIUS")?)?;
            (r, r)
        } else {
            (
                number(attr(v, "SEMI_AXIS_1")?)?,
                number(attr(v, "SEMI_AXIS_2")?)?,
            )
        };
        if !(a > 0. && b > 0. && a.is_finite() && b.is_finite()) {
            return None;
        }
        Curve2::Conic { centre, x, a, b }
    } else if is(v, "B_SPLINE_CURVE") {
        let degree = usize::try_from(match attr(v, "DEGREE")?.kind {
            ValueKind::Integer(d) => d,
            _ => return None,
        })
        .ok()
        .filter(|d| (1..=32).contains(d))?;
        let ValueKind::Aggregate(points) = &attr(v, "CONTROL_POINTS_LIST")?.kind else {
            return None;
        };
        let controls: Vec<[f64; 2]> = points
            .iter()
            .map(|p| point2(reference(p)?))
            .collect::<Option<_>>()?;
        let n = controls.len();
        let spans = n.checked_sub(degree).filter(|&s| s > 0)?;
        let knots: Vec<f64> = if is(v, "B_SPLINE_CURVE_WITH_KNOTS") {
            let multiplicities = list(attr(v, "KNOT_MULTIPLICITIES")?)?;
            let values = list(attr(v, "KNOTS")?)?;
            if multiplicities.len() != values.len() {
                return None;
            }
            let mut knots = Vec::new();
            for (m, k) in multiplicities.iter().zip(values) {
                if !(*m >= 1. && *m <= degree as f64 + 1. && m.fract() == 0.) {
                    return None;
                }
                knots.extend(std::iter::repeat_n(k, *m as usize));
            }
            knots
        } else if is(v, "QUASI_UNIFORM_CURVE") {
            let mut knots = vec![0.; degree + 1];
            knots.extend((1..spans).map(|i| i as f64));
            knots.extend(std::iter::repeat_n(spans as f64, degree + 1));
            knots
        } else if is(v, "UNIFORM_CURVE") {
            (0..n + degree + 1)
                .map(|i| i as f64 - degree as f64)
                .collect()
        } else if is(v, "BEZIER_CURVE") {
            if (n - 1) % degree != 0 {
                return None;
            }
            let segments = (n - 1) / degree;
            let mut knots = vec![0.; degree + 1];
            for i in 1..segments {
                knots.extend(std::iter::repeat_n(i as f64, degree));
            }
            knots.extend(std::iter::repeat_n(segments as f64, degree + 1));
            knots
        } else {
            return None;
        };
        if knots.len() != n + degree + 1 {
            return None;
        }
        let weights = if is(v, "RATIONAL_B_SPLINE_CURVE") {
            list(attr(v, "WEIGHTS_DATA")?)?
        } else {
            vec![1.; n]
        };
        if weights.len() != n {
            return None;
        }
        Curve2::Nurbs {
            degree,
            knots,
            controls,
            weights,
        }
    } else {
        return None;
    };
    Some(Supplied { surface, curve })
}

/// Map a supplied curve into chart coordinates `scale * step + offset` per axis, with
/// the edge parameter `t = parameter_scale * t_step`; `angle` is radians per
/// plane-angle unit. None when the image is not representable exactly (a conic
/// under a nonuniform map or a reparameterization).
pub(crate) fn to_chart(
    curve: &Curve2,
    [scale, offset]: [[f64; 2]; 2],
    parameter_scale: f64,
    angle: f64,
) -> Option<CurveGeometry<ParameterSpace, 2>> {
    let map = |p: [f64; 2]| [scale[0] * p[0] + offset[0], scale[1] * p[1] + offset[1]];
    let point = |p: [f64; 2]| Point::<ParameterSpace, 2>::new(map(p)).ok();
    match curve {
        Curve2::Line { origin, direction } => Some(CurveGeometry::Analytic(
            Curve::line(
                point(*origin)?,
                Vector::new([
                    scale[0] * direction[0] / parameter_scale,
                    scale[1] * direction[1] / parameter_scale,
                ])
                .ok()?,
            )
            .ok()?,
        )),
        Curve2::Conic { centre, x, a, b } => {
            // Only a similarity keeps the conic an ellipse in an orthonormal frame, and
            // only an angle parameter equal to the edge parameter keeps its speed.
            if scale[0] != scale[1] || scale[0] <= 0. || (angle - parameter_scale).abs() > 1e-15 {
                return None;
            }
            let frame = PlaneFrame::new(
                point(*centre)?,
                Vector::new(*x).ok()?,
                Vector::new([-x[1], x[0]]).ok()?,
                NumericalTolerance::default(),
            )
            .ok()?;
            Some(CurveGeometry::Analytic(
                Curve::ellipse(
                    frame,
                    Length::metres(scale[0] * a).ok()?,
                    Length::metres(scale[0] * b).ok()?,
                )
                .ok()?,
            ))
        }
        Curve2::Nurbs {
            degree,
            knots,
            controls,
            weights,
        } => {
            let controls: Vec<_> = controls.iter().map(|&p| point(p)).collect::<Option<_>>()?;
            let knots: Vec<f64> = knots.iter().map(|k| k * parameter_scale).collect();
            Some(CurveGeometry::Nurbs(
                NurbsCurve::new(*degree, &knots, &controls, weights, SplineLimits::default())
                    .ok()?,
            ))
        }
    }
}
